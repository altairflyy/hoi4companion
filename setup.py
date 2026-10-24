"""The wheel contains the semantic host; end users need neither SDK nor Rust."""
from pathlib import Path
import runpy
from setuptools import setup
from setuptools.command.build_py import build_py


class BuildWithCwtools(build_py):
    def run(self):
        super().run()
        builder=runpy.run_path(str(Path(__file__).parent/'scripts/build-cwtools.py'))
        destination=(Path(__file__).parent/'src' if getattr(self,'editable_mode',False) else Path(self.build_lib))
        builder['build'](destination/'hoi4companion/_native/cwtools')


setup(cmdclass={'build_py':BuildWithCwtools})
