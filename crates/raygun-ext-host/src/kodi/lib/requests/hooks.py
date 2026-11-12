"""`requests.hooks`."""

HOOKS = ['response']


def default_hooks():
    return {event: [] for event in HOOKS}


def dispatch_hook(key, hooks, hook_data, **kwargs):
    hooks = hooks or {}
    found = hooks.get(key)
    if found:
        if hasattr(found, '__call__'):
            found = [found]
        for hook in found:
            answer = hook(hook_data, **kwargs)
            if answer is not None:
                hook_data = answer
    return hook_data
