from collections.abc import Callable
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Any, TypeVar, cast
from uuid import UUID

import dateutil.parser

T = TypeVar("T")
EnumT = TypeVar("EnumT", bound=Enum)


def from_str(x: Any) -> str:
    assert isinstance(x, str)
    return x


def from_list(f: Callable[[Any], T], x: Any) -> list[T]:
    assert isinstance(x, list)
    return [f(y) for y in x]


def from_datetime(x: Any) -> datetime:
    return dateutil.parser.parse(x)


def from_none(x: Any) -> Any:
    assert x is None
    return x


def from_union(fs, x):
    for f in fs:
        try:
            return f(x)
        except Exception:  # noqa: BLE001, S112
            continue
    assert False


def to_enum(c: type[EnumT], x: Any) -> EnumT:
    assert isinstance(x, c)
    return x.value


def to_class(c: type[T], x: Any) -> dict:
    assert isinstance(x, c)
    return cast(Any, x).to_dict()


@dataclass
class SignedApproval:
    id: UUID
    signature: str

    @staticmethod
    def from_dict(obj: Any) -> 'SignedApproval':
        assert isinstance(obj, dict)
        id = UUID(obj.get("id"))
        signature = from_str(obj.get("signature"))
        return SignedApproval(id, signature)

    def to_dict(self) -> dict:
        result: dict = {}
        result["id"] = str(self.id)
        result["signature"] = from_str(self.signature)
        return result


class Kind(Enum):
    CREATE = "create"
    DELETE = "delete"
    MODIFY = "modify"


@dataclass
class CustodianProto:
    id: UUID
    kind: Kind
    paths: list[str]
    timestamp: datetime
    approvals: list[SignedApproval] | None = None
    hash_after: str | None = None
    hash_before: str | None = None

    @staticmethod
    def from_dict(obj: Any) -> 'CustodianProto':
        assert isinstance(obj, dict)
        id = UUID(obj.get("id"))
        kind = Kind(obj.get("kind"))
        paths = from_list(from_str, obj.get("paths"))
        timestamp = from_datetime(obj.get("timestamp"))
        approvals = from_union([lambda x: from_list(SignedApproval.from_dict, x), from_none], obj.get("approvals"))
        hash_after = from_union([from_str, from_none], obj.get("hash_after"))
        hash_before = from_union([from_str, from_none], obj.get("hash_before"))
        return CustodianProto(id, kind, paths, timestamp, approvals, hash_after, hash_before)

    def to_dict(self) -> dict:
        result: dict = {}
        result["id"] = str(self.id)
        result["kind"] = to_enum(Kind, self.kind)
        result["paths"] = from_list(from_str, self.paths)
        result["timestamp"] = self.timestamp.isoformat()
        if self.approvals is not None:
            result["approvals"] = from_union([lambda x: from_list(lambda x: to_class(SignedApproval, x), x), from_none], self.approvals)
        if self.hash_after is not None:
            result["hash_after"] = from_union([from_str, from_none], self.hash_after)
        if self.hash_before is not None:
            result["hash_before"] = from_union([from_str, from_none], self.hash_before)
        return result


def custodian_proto_from_dict(s: Any) -> CustodianProto:
    return CustodianProto.from_dict(s)


def custodian_proto_to_dict(x: CustodianProto) -> Any:
    return to_class(CustodianProto, x)
