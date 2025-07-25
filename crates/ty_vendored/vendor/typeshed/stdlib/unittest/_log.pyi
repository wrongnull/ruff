import logging
import sys
from types import TracebackType
from typing import ClassVar, Generic, NamedTuple, TypeVar
from unittest.case import TestCase, _BaseTestCaseContext

_L = TypeVar("_L", None, _LoggingWatcher)

class _LoggingWatcher(NamedTuple):
    """_LoggingWatcher(records, output)"""

    records: list[logging.LogRecord]
    output: list[str]

class _AssertLogsContext(_BaseTestCaseContext, Generic[_L]):
    """A context manager for assertLogs() and assertNoLogs()"""

    LOGGING_FORMAT: ClassVar[str]
    logger_name: str
    level: int
    msg: None
    if sys.version_info >= (3, 10):
        def __init__(self, test_case: TestCase, logger_name: str, level: int, no_logs: bool) -> None: ...
        no_logs: bool
    else:
        def __init__(self, test_case: TestCase, logger_name: str, level: int) -> None: ...

    def __enter__(self) -> _L: ...
    def __exit__(
        self, exc_type: type[BaseException] | None, exc_value: BaseException | None, tb: TracebackType | None
    ) -> bool | None: ...
