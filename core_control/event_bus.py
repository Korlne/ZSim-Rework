"""
事件总线与Tick驱动器模块
主要功能：作为全局时间轴推进器，按照纯逐Tick推进的方式运行模拟，并向外广播 Tick 事件。
"""
import logging
from .dispatcher import on_tick, on_combat_end
from .game_state import GameState

logger = logging.getLogger("zsim.Core.EventBus")

class EventBus:
    """
    EventBus 类
    主要功能：管理模拟的生命周期，按Tick循环推进时间轴，并触发底层事件以驱动其他计算逻辑。
    """
    def __init__(self, state: GameState):
        # 初始化时接收并持有具有校验功能的全局状态快照
        self.state = state

    def run_simulation(self):
        """启动并执行Tick循环推进过程"""
        logger.info(f"Simulation started. Max ticks: {self.state.max_ticks}")
        
        # 只要处于运行状态且未到达Tick数上限，即持续推进
        while self.state.is_running and self.state.current_tick < self.state.max_ticks:
            current = self.state.current_tick
            logger.debug(f"[TICK] Processing tick: {current}")
            
            # 广播 Tick 事件，传递 tick 参数，下游系统将监听此事件
            on_tick.send(self, tick=current)
            
            # 每执行一次循环，时间轴向前推进一Tick
            self.state.current_tick += 1

        # 循环结束，判断并记录结束原因
        reason = "Max ticks reached" if self.state.current_tick >= self.state.max_ticks else "Simulation stopped"
        logger.info(f"Simulation ended at tick {self.state.current_tick}. Reason: {reason}")
        
        # 广播战斗结束事件
        on_combat_end.send(self, reason=reason)