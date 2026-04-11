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