"""
全链路集成验证脚本
主要功能：组装 GameState, EventBus 和 APLManager，读取真实的 JSON 排轴，生成完整的模拟日志。
"""
import logging
from core_control.logger import setup_logging
from core_control.game_state import GameState
from core_control.event_bus import EventBus
from core_control.apl_manager import APLManager
from core_control.interfaces import IResourceValidator
from core_control.dispatcher import on_tick

# 1. 定义一个简单的资源验证器
class SimpleValidator(IResourceValidator):
    def can_execute(self, action_id: str, state: GameState) -> bool:
        # 模拟：允许普攻，但遇到技能时直接拒绝，以此触发我们的错误捕获机制
        if "Attack" in action_id:
            return True
        return False

def main():
    # 2. 初始化集中式日志
    setup_logging()
    logger = logging.getLogger("zsim.Main")
    logger.info("=== 初始化 ZSim 模拟引擎 ===")

    # 3. 初始化核心组件，设置最多跑 10 Tick
    state = GameState(max_ticks=10)
    bus = EventBus(state)
    validator = SimpleValidator()
    manager = APLManager(validator)

    # 4. 加载我们刚刚创建的 JSON 排轴文件
    manager.load_from_json("Endaxis_Timeline_2026-04-03.json")

    # 5. 编写 Tick 监听器：每次 Tick 尝试处理一个动作
    def handle_tick(sender, tick):
        try:
            manager.process_next_action(state)
        except Exception:
            # 异常已被 @emit_on_error 装饰器广播并安全接管
            # 此时我们模拟“遇到致命错误，停止运行”的逻辑
            logger.warning("接收到致命错误，正在停止时间轴推进...")
            state.is_running = False

    # 6. 将监听器挂载到全局 on_tick 信号上
    on_tick.connect(handle_tick)

    # 7. 启动事件总线
    logger.info("=== 开始执行时间轴 ===")
    bus.run_simulation()
    logger.info("=== 模拟结束 ===")

if __name__ == "__main__":
    main()