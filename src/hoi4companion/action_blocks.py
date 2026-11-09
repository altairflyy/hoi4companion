"""Native source positions for action blocks; presentation, never runtime gates."""

def annotate(guide):
    from .backend_core import _request,_publish_in_place
    result=_request(0,'import_action_blocks',guide=guide)
    _publish_in_place(guide,result['guide'])
    return result['changes']
