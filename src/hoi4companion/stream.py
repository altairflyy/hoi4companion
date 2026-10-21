"""One bounded, lazy state producer shared by all local SSE subscribers."""
from copy import deepcopy
from dataclasses import dataclass
import json
import threading
import time


class SubscriberLimitError(RuntimeError):
    pass


@dataclass(frozen=True)
class LiveEvent:
    revision: int
    value: dict


def state_signature(value):
    """Transport/liveness measurements do not constitute a gameplay change."""
    ignored = {'observed_at', 'age_seconds', 'native_tick', 'metrics', 'checked_at'}

    def semantic(item):
        if isinstance(item, dict):
            return {key: semantic(child) for key, child in item.items() if key not in ignored}
        if isinstance(item, (list, tuple)):
            return [semantic(child) for child in item]
        return item

    return json.dumps(semantic(value), sort_keys=True, separators=(',', ':'), ensure_ascii=False)


class Subscription:
    def __init__(self, hub):
        self.hub, self.revision, self.closed = hub, -1, False

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def wait(self, timeout=5):
        """Return the newest state; intermediate revisions may be coalesced."""
        hub = self.hub
        with hub._condition:
            hub._expire_if_stale_locked()
            hub._condition.wait_for(
                lambda: self.closed or hub._closed or
                (hub._event is not None and hub._event.revision > self.revision), timeout)
            hub._expire_if_stale_locked()
            if self.closed or hub._closed or hub._event is None or hub._event.revision <= self.revision:
                return None
            self.revision = hub._event.revision
            return LiveEvent(self.revision, deepcopy(hub._event.value))

    def close(self):
        with self.hub._condition:
            self.closed = True
            self.hub._clients.discard(self)
            self.hub._condition.notify_all()


class LiveEventHub:
    def __init__(self, source, interval=.25, max_subscribers=8, on_idle=None):
        if interval <= 0 or max_subscribers < 1:
            raise ValueError('Positive interval and subscriber limit required')
        self.source, self.interval, self.max_subscribers = source, interval, max_subscribers
        self.on_idle = on_idle
        self._condition = threading.Condition()
        self._clients, self._event = set(), None
        self._signature, self._revision, self._last_check = None, 0, None
        self._worker, self._closed = None, False

    @property
    def closed(self):
        with self._condition:
            return self._closed

    def subscribe(self):
        with self._condition:
            if self._closed:
                raise RuntimeError('State stream closed')
            if len(self._clients) >= self.max_subscribers:
                raise SubscriberLimitError('Too many state stream subscribers')
            if not self._clients:
                # A resumed stream must wait for a fresh read, never revive old data.
                self._event, self._signature, self._last_check = None, None, None
            client = Subscription(self)
            self._clients.add(client)
            if self._worker is None:
                self._worker = threading.Thread(target=self._run, name='hoi4-live-state', daemon=True)
                self._worker.start()
            self._condition.notify_all()
            return client

    def heartbeat(self):
        with self._condition:
            self._expire_if_stale_locked()
            age = time.monotonic() - self._last_check if self._last_check is not None else None
            value = self._event.value if self._event is not None else {}
            fresh = age is not None and age < max(5, self.interval * 8)
            return {'revision': self._revision, 'usable': bool(fresh and value.get('usable')),
                    'status': value.get('status', 'connecting') if fresh else 'stale',
                    'age_seconds': round(age, 3) if age is not None else None}

    def _expire_if_stale_locked(self):
        """Reader liveness is part of state validity, independent of transport."""
        if (self._event is None or not self._event.value.get('usable') or self._last_check is None or
                time.monotonic() - self._last_check < max(5, self.interval * 8)):
            return
        value = {**self._event.value, 'usable': False, 'status': 'stale',
                 'error': 'Il lettore non conferma più uno stato fresco'}
        self._revision += 1
        self._event = LiveEvent(self._revision, value)
        # The next successful source result must be delivered even if gameplay
        # remained identical throughout the interruption.
        self._signature = None
        self._condition.notify_all()

    def _run(self):
        while True:
            with self._condition:
                idle = self._closed or not self._clients
            if idle:
                # Keep ownership of the worker while releasing the reader. A new
                # subscriber may arrive during cleanup; that same worker resumes.
                if self.on_idle is not None:
                    try:
                        self.on_idle()
                    except Exception:
                        pass
                with self._condition:
                    if self._closed or not self._clients:
                        self._worker = None
                        self._condition.notify_all()
                        return
                continue
            started = time.monotonic()
            try:
                value = deepcopy(self.source())
                signature = state_signature(value)
            except Exception as exc:
                # Unexpected reader failures are visible as unusable state, not a dead stream.
                value = {'usable': False, 'status': 'disconnected', 'sample': None,
                         'error': 'Lettore locale non disponibile: ' + str(exc)}
                signature = state_signature(value)
            with self._condition:
                self._expire_if_stale_locked()
                self._last_check = time.monotonic()
                if signature != self._signature:
                    self._signature = signature
                    self._revision += 1
                    self._event = LiveEvent(self._revision, value)
                    self._condition.notify_all()
                remaining = max(0, self.interval - (time.monotonic() - started))
                self._condition.wait_for(lambda: self._closed or not self._clients, remaining)

    def close(self):
        with self._condition:
            self._closed = True
            self._condition.notify_all()
            worker = self._worker
        if worker is not None and worker is not threading.current_thread():
            worker.join(timeout=1)
