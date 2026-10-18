"""Internal page draft values shared by document extractors."""

from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class PageDraft:
    index: int
    page: dict
    regions: List[dict] = field(default_factory=list)
    objects: List[dict] = field(default_factory=list)
    search_units: List[dict] = field(default_factory=list)
    processed_for_search: bool = False
    coverage_unprocessed: List[dict] = field(default_factory=list)
    coverage_failed: List[dict] = field(default_factory=list)
    preview_resource_id: Optional[str] = None
    formula_evidence: List[dict] = field(default_factory=list)

    @property
    def page_id(self) -> str:
        return self.page["page_id"]

    @property
    def page_status(self) -> str:
        return self.page["status"]
