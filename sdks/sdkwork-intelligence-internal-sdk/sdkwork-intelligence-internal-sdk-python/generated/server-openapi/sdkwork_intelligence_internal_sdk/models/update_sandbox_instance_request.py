from __future__ import annotations
from dataclasses import dataclass
from typing import TYPE_CHECKING, Optional, List, Dict, Any


@dataclass
class UpdateSandboxInstanceRequest:
    sandbox_instance_name: Optional[str] = None
