"""
APL解析与动作管理器模块
主要功能：读取并解析JSON格式的动作排轴文件，应用面向切面装饰器进行错误处理，将动作与验证器结合执行。
"""
import json
import logging
from typing import List, Dict, Optional
from .interfaces import IResourceValidator
from .dispatcher import on_action_request, on_action_start
from .decorators import emit_on_error
from .exceptions import ActionExecutionError

logger = logging.getLogger("zsim.Core.APLManager")

class APLManager:
    """
    APLManager 类
    主要功能：动作逻辑流与解析控制器。读取外部JSON队伍排轴序列，按序向验证器发起放行判断并执行。
    """
    def __init__(self, validator: IResourceValidator):
        # 依赖注入：注入动作验证器，用于后续的动作可行性判定
        self.validator = validator
        self.action_queue: List[Dict] = []
        self.last_successful_action_id: Optional[str] = None

    def load_from_json(self, file_path: str):
        """加载排轴JSON文件并解析数据结构"""
        logger.info(f"Loading APL from: {file_path}")
        with open(file_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
            # 根据约定结构提取 actions 队列
            self.action_queue = data.get("scenarioList", [])[0].get("data", {}).get("tracks", [])[0].get("actions", [])
        logger.debug(f"Loaded {len(self.action_queue)} actions.")

    @emit_on_error
    def process_next_action(self, state: 'GameState'):
        """
        请求执行下一个动作。
        使用 @emit_on_error 装饰器，任何产生的异常均由此装饰器捕获并进行全局广播。
        """
        # 队列为空时停止处理
        if not self.action_queue:
            return False

        # 弹出序列首个动作并获取标识ID
        next_action = self.action_queue.pop(0)
        action_id = next_action.get("id")
        
        logger.debug(f"Requesting action execution: {action_id}")
        # 发送动作请求信号
        on_action_request.send(self, action_id=action_id)

        # 向注入的判断类发起资源与逻辑可行性判定
        if self.validator.can_execute(action_id, state):
            # 判定通过，更新状态并广播开始执行信号
            self.last_successful_action_id = action_id
            logger.info(f"Action validated and started: {action_id}")
            on_action_start.send(self, action_id=action_id)
            return True
        else:
            # 判定拒绝，携带当前失败指令与上一成功指令的上下文抛出专属异常
            error_msg = f"当前指令 {action_id} 无法释放，前一个指令是 {self.last_successful_action_id}"
            # 抛出后由 @emit_on_error 自动接管处理
            raise ActionExecutionError(error_msg)