"""
切面逻辑装饰器模块
主要功能：提供用于AOP(面向切面编程)的装饰器，负责自动捕获执行异常，通过 blinker 进行全局广播，并安全向上抛出。
"""
import functools
import logging
from typing import Callable, Any
from .dispatcher import on_error_raised

logger = logging.getLogger("zsim.Core.Decorators")

def emit_on_error(func: Callable) -> Callable:
    """装饰器：自动捕获执行异常，通过 blinker 广播，并安全向上抛出"""
    @functools.wraps(func)
    def wrapper(self, *args, **kwargs) -> Any:
        try:
            # 尝试执行被装饰的原函数业务逻辑
            return func(self, *args, **kwargs)
        except Exception as e:
            # 发生异常时，首先记录日志
            logger.error(f"Error captured in {func.__name__}: {str(e)}")
            # 通过全局事件总线广播该异常，通知其他监听模块（例如日志、前端上报）
            on_error_raised.send(self, error=e)
            # 安全地将异常抛出，中断当前流程
            raise e
    return wrapper