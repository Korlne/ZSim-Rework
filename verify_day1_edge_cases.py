import logging
import traceback
from pydantic import ValidationError
from core_control.game_state import GameState
from core_control.interfaces import IResourceValidator
from core_control.logger import setup_logging

def run_edge_cases_verification():
    print("=== 开始 Day 1 深度边缘场景排查 ===\n")

    # 场景 1: Pydantic 隐式类型转换测试
    print("[排查 1] Pydantic 隐式类型转换 (传入字符串 '15')")
    try:
        state_coerced = GameState(current_frame="15")
        print(f" -> [警告] 发生了隐式转换！传入字符串 '15'，被静默转换为: {type(state_coerced.current_frame)}，值为: {state_coerced.current_frame}")
        print("    (隐患：如果上游传错类型，系统不会报错，可能导致隐蔽的数值计算Bug)")
    except ValidationError:
        print(" -> [安全] 成功拦截了错误的类型输入。")

    # 场景 2: GameState 默认值测试
    print("\n[排查 2] GameState 默认值完整性测试 (不传参数)")
    state_default = GameState()
    if state_default.current_frame == 0 and state_default.max_frames == 18000 and state_default.is_running is True:
        print(" -> [安全] 默认值完全符合大纲要求。")
    else:
        print(f" -> [错误] 默认值异常！当前值为: {state_default}")

    # 场景 3: abc 接口参数盲区测试
    print("\n[排查 3] abc 接口签名的 '参数盲区' 测试")
    class BadValidator(IResourceValidator):
        # 故意写错参数列表，漏掉 action_id 和 state
        def can_execute(self):
            return True
            
    try:
        # 这里实例化不会报错，因为 abc 只检查方法名
        bad_validator = BadValidator()
        print(" -> [警告] 实例化通过！系统允许创建参数签名错误的子类。")
        
        # 模拟 Day 2 实际调用时的场景
        print(" -> 尝试模拟实际调用: validator.can_execute('attack', state_default)")
        bad_validator.can_execute('attack', state_default)
    except TypeError as e:
        print(f" -> [崩溃] 运行时发生 TypeError: {e}")
        print("    (隐患：abc 无法在加载期拦截参数错误，Bug 被推迟到了运行期)")

    # 场景 4: 中文字符集日志输出测试
    print("\n[排查 4] 复杂中文字符日志写入测试")
    setup_logging()
    logger = logging.getLogger("zsim.Core.Test")
    test_str = "边缘测试：属性Tag包含 [特殊以太-轩墨] / [特殊冰-烈霜]"
    logger.debug(test_str)
    print(f" -> [已执行] 成功向日志发送测试文本。")
    print("    (请务必打开 logs 文件夹下的最新日志文件，确认是否出现乱码)")

if __name__ == "__main__":
    run_edge_cases_verification()