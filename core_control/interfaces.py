"""
核心接口模块
主要功能：定义系统的抽象基类，使用 @abstractmethod 强制子类实现特定方法，遵循依赖倒置原则 (DIP)。
"""
from abc import ABC, abstractmethod
from typing import Any

class IResourceValidator(ABC):
    """资源验证器抽象基类，作为后续技能与能量综合判断类的接口约定"""
    
    @abstractmethod
    def can_execute(self, action_id: str, state: Any) -> bool:
        """
        校验给定动作在当前状态下是否允许执行
        注意：子类必须实现此方法，否则实例化时将抛出 TypeError
        """
        pass