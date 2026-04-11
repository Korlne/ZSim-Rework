"""
全局状态快照模块
主要功能：基于 Pydantic 构建强类型的全局数据中心，提供带有严格校验机制的上下文快照。
"""
from pydantic import BaseModel, Field, field_validator, ConfigDict

class GameState(BaseModel):
    """
    全局状态模型
    承载当前帧数、最大帧数与运行状态，作为各系统间传递的数据对象。
    遵循1秒60帧
    """
    
    # 开启严格模式，拒绝任何隐式类型转换 (如将字符串 "15" 转为整数 15)
    model_config = ConfigDict(strict=True)
    
    # 使用 Field 声明默认值与字段描述
    current_frame: int = Field(default=0, description="当前模拟帧数")
    max_frames: int = Field(default=18000, description="最大限制帧数")
    is_running: bool = Field(default=True, description="模拟器运行状态")

    @field_validator('current_frame', 'max_frames')
    @classmethod
    def check_non_negative(cls, value: int) -> int:
        """
        字段校验装饰器：拦截负数帧赋值
        对 current_frame 和 max_frames 字段进行检查。
        """
        # 如果传入的数值小于0，主动抛出 ValueError 以阻断实例化过程
        if value < 0:
            raise ValueError("帧数不能为负数")
        return value