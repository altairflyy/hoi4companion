"""Acquisition adapter for the Rust-owned shared state/event hub.

Only the acquisition callback and its lazy worker remain transitional Python.
Rust owns revisions, semantic deduplication, subscriber admission, coalescing,
liveness expiry and waiting. Diagnostic timestamps cannot refresh evidence.
"""
from dataclasses import dataclass
import threading
import time
from .backend_core import NativeEvents,state_signature


class SubscriberLimitError(RuntimeError):pass


@dataclass(frozen=True)
class LiveEvent:
    revision:int
    value:dict


class Subscription:
    def __init__(self,hub,identity):self.hub,self.identity,self.closed=hub,identity,False
    def __enter__(self):return self
    def __exit__(self,*_):self.close()
    def wait(self,timeout=5):
        value=self.hub._native.wait(self.identity,timeout,self.hub.clock())
        return LiveEvent(value['revision'],value['value']) if value is not None else None
    def close(self):
        with self.hub._condition:
            if self.closed:return
            self.closed=True;self.hub._native.unsubscribe(self.identity)
            self.hub._clients.discard(self);self.hub._condition.notify_all()


class LiveEventHub:
    def __init__(self,source,interval=.25,max_subscribers=8,on_idle=None,*,clock=time.monotonic):
        self._native=NativeEvents(interval,max_subscribers)
        self.source,self.interval,self.max_subscribers=source,interval,max_subscribers
        self.on_idle,self.clock=on_idle,clock
        self._condition=threading.Condition()
        self._clients=set();self._worker=None;self._closed=False

    @property
    def closed(self):
        with self._condition:return self._closed

    @property
    def _event(self):
        value=self._native.peek(self.clock())
        return LiveEvent(value['revision'],value['value']) if value is not None else None

    def subscribe(self):
        with self._condition:
            try:identity=self._native.subscribe(self.clock())
            except ValueError as exc:
                if 'Too many' in str(exc):raise SubscriberLimitError(str(exc)) from exc
                if 'closed' in str(exc):raise RuntimeError(str(exc)) from exc
                raise
            client=Subscription(self,identity);self._clients.add(client)
            if self._worker is None:
                self._worker=threading.Thread(target=self._run,name='hoi4-live-state',daemon=True)
                self._worker.start()
            self._condition.notify_all();return client

    def heartbeat(self):return self._native.heartbeat(self.clock())

    def _run(self):
        while True:
            with self._condition:idle=self._closed or not self._clients
            if idle:
                if self.on_idle is not None:
                    try:self.on_idle()
                    except Exception:pass
                with self._condition:
                    if self._closed or not self._clients:
                        self._worker=None;self._condition.notify_all();return
                continue
            started=time.monotonic()
            try:self._native.publish(self.source(),self.clock())
            except Exception as exc:
                self._native.publish({'usable':False,'status':'disconnected','sample':None,
                    'error':'Lettore locale non disponibile: '+str(exc)},self.clock())
            with self._condition:
                remaining=max(0,self.interval-(time.monotonic()-started))
                self._condition.wait_for(lambda:self._closed or not self._clients,remaining)

    def close(self):
        with self._condition:
            self._closed=True;self._native.stop();self._condition.notify_all();worker=self._worker
        if worker is not None and worker is not threading.current_thread():worker.join(timeout=1)
