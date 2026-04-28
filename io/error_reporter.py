# io/error_reporter.py
"""
报错管理器模块
主要功能：统一捕获各模块异常，输出友好调试信息并安全中断模拟。
承载当前分散在 decorators 中的报错逻辑，提供结构化的错误报告。
"""
import json
import logging
from enum import Enum
from pathlib import Path
from typing import Optional

logger = logging.getLogger("zsim.IO.ErrorReporter")


class ErrorCategory(str, Enum):
    """错误分类枚举"""
    ENERGY_INSUFFICIENT = "energy_insufficient"
    RESOURCE_INSUFFICIENT = "resource_insufficient"
    ACTION_NOT_IN_DICTIONARY = "action_not_in_dictionary"
    APL_UNKNOWN_ENTITY = "apl_unknown_entity"
    DATA_LOAD_FAILURE = "data_load_failure"
    NUMERIC_OVERFLOW = "numeric_overflow"
    UNKNOWN = "unknown"


class ErrorReport:
    """
    结构化错误报告
    包含错误分类、发生帧、来源模块、消息与上下文信息。
    """

    def __init__(
        self,
        category: ErrorCategory,
        tick: int,
        source_module: str,
        message: str,
        current_action: str = "",
        last_successful_action: str = "",
        context: Optional[dict] = None,
    ):
        self.category = category
        self.tick = tick
        self.source_module = source_module
        self.message = message
        self.current_action = current_action
        self.last_successful_action = last_successful_action
        self.context = context or {}

    def to_dict(self) -> dict:
        return {
            "error_type": self.category.value,
            "tick": self.tick,
            "source_module": self.source_module,
            "message": self.message,
            "context": {
                "current_action": self.current_action,
                "last_successful_action": self.last_successful_action,
                **self.context,
            },
        }


class ErrorReporter:
    """
    ErrorReporter 类
    主要功能：监听 on_error_raised 事件，分类处理异常并生成结构化报告。
    集成到现有 emit_on_error 装饰器的错误广播流程。
    """

    def __init__(self, output_dir: str = "logs"):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.reports: list = []

    def handle_error(self, error: Exception, **kwargs):
        """处理由 emit_on_error 广播的错误"""
        from core_control.exceptions import ResourceValidationError, ActionExecutionError

        if isinstance(error, ResourceValidationError):
            category = ErrorCategory.RESOURCE_INSUFFICIENT
            context = {
                "missing_resource": error.missing_resource,
                "current_value": error.current_value,
                "required_value": error.required_value,
            }
        elif isinstance(error, ActionExecutionError):
            category = ErrorCategory.ACTION_NOT_IN_DICTIONARY
            context = {}
        else:
            category = ErrorCategory.UNKNOWN
            context = {}

        report = ErrorReport(
            category=category,
            tick=kwargs.get("tick", 0),
            source_module=type(error).__module__,
            message=str(error),
            current_action=kwargs.get("action_id", ""),
            context=context,
        )
        self.reports.append(report)
        logger.error(f"[{category.value}] {error}")

    def register_event_handlers(self):
        """注册到 EventBus"""
        from core_control.dispatcher import on_error_raised

        def handler(sender, error, **kwargs):
            self.handle_error(error, **kwargs)

        on_error_raised.connect(handler, weak=False)

    def get_report_summary(self) -> str:
        """生成错误摘要"""
        return json.dumps(
            [r.to_dict() for r in self.reports],
            ensure_ascii=False, indent=2,
        )
