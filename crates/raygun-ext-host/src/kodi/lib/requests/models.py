"""`requests.models`."""

from . import PreparedRequest, Request, Response  # noqa: F401

REDIRECT_STATI = (301, 302, 303, 307, 308)
DEFAULT_REDIRECT_LIMIT = 30
