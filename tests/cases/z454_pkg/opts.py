from typing import TYPE_CHECKING

if TYPE_CHECKING:
    # (reqs reads index_url at import: imported here it would find it not set yet)
    from reqs import Parsed
    mode = "checking"
else:
    mode = "running"

index_url = "--index-url"
