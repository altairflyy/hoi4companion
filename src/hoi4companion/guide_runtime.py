"""Universal Rust runtime; development reference is explicitly opt-in only."""
from .contracts import GuideEvaluation,GuideProgress


def evaluate(guide,state,progress: GuideProgress|None=None)->GuideEvaluation:
    from .guide_core import evaluate as evaluate_native
    return evaluate_native(guide,state,progress)


def evaluate_python(guide,state,progress: GuideProgress|None=None)->GuideEvaluation:
    """Independent developer oracle, never a missing-backend fallback."""
    from ._guide_runtime_reference import evaluate_python as reference
    return reference(guide,state,progress)
