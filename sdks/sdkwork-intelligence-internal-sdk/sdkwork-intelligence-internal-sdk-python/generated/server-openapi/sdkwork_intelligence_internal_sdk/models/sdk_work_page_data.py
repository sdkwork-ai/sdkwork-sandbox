from __future__ import annotations
from dataclasses import dataclass
from typing import TYPE_CHECKING, Optional, List, Dict, Any

if TYPE_CHECKING:
    from .page_info import PageInfo
    from .sandbox_instance import SandboxInstance


@dataclass
class SdkWorkPageData:
    items: List[SandboxInstance]
    page_info: PageInfo
