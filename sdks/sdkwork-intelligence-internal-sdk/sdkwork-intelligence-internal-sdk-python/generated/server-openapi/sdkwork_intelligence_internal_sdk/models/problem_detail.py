from __future__ import annotations
from dataclasses import dataclass
from typing import TYPE_CHECKING, Optional, List, Dict, Any


@dataclass
class ProblemDetail:
    type: str
    title: str
    status: int
    code: int
    trace_id: str
    detail: Optional[str] = None
    instance: Optional[str] = None
    operation_id: Optional[str] = None
