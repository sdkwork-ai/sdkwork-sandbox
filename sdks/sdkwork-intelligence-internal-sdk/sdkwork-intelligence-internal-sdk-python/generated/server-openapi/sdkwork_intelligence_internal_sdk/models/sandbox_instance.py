from __future__ import annotations
from dataclasses import dataclass
from typing import TYPE_CHECKING, Optional, List, Dict, Any


@dataclass
class SandboxInstance:
    sandbox_instance_id: str
    sandbox_instance_state: str
    sandbox_version: str
    sandbox_instance_name: Optional[str] = None
    sandbox_instance_owner_id: Optional[str] = None
    sandbox_instance_created_at: Optional[str] = None
    sandbox_instance_expires_at: Optional[str] = None
