"""
自定义异常模块
主要功能：定义 ZSim 核心控制层需要抛出的各类专属异常体系。
"""

class ZSimBaseException(Exception):
    """ZSim 基础异常类，作为所有自定义异常的父类"""
    pass

class ActionExecutionError(ZSimBaseException):
    """动作执行失败异常，用于动作校验不通过时抛出"""
    pass


class ResourceValidationError(ZSimBaseException):
    """资源校验失败异常，用于资源与条件不满足时抛出"""
    def __init__(self, action_id: str, missing_resource: str,
                 current_value: float, required_value: float,
                 last_successful_action: str = ""):
        self.action_id = action_id
        self.missing_resource = missing_resource
        self.current_value = current_value
        self.required_value = required_value
        self.last_successful_action = last_successful_action
        msg = (
            f"资源校验失败: 动作 {action_id} 需要 {missing_resource}={required_value}, "
            f"当前值={current_value}"
        )
        if last_successful_action:
            msg += f", 上一成功指令={last_successful_action}"
        super().__init__(msg)