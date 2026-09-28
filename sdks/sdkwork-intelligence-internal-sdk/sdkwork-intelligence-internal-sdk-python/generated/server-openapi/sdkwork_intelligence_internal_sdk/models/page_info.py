from __future__ import annotations
from dataclasses import dataclass
from typing import TYPE_CHECKING, Optional, List, Dict, Any


@dataclass
class PageInfo:
    mode: str
    page_size: Optional[int] = None
    next_cursor: Optional[str] = None
    has_more: Optional[bool] = None
