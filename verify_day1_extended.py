import logging
from pydantic import ValidationError
from core_control.game_state import GameState
from core_control.interfaces import IResourceValidator
from core_control.exceptions import ActionExecutionError
from core_control.logger import setup_logging

def run_extended_verification():
    print("=== 开始 Day 1 扩展边缘场景验证 ===\n")

    # 场景 1: 验证 max_frames 的非负数拦截 (对应文档 3.4)
    print("[测试 1] 验证 max_frames 负数拦截")
    try:
        # 尝试传入非法的 max_frames
        state = GameState(max_frames=-500)
    except ValidationError as e:
        print(" -> [成功] 拦截了 max_frames 的负数输入。")
    else:
        print(" -> [失败] 未拦截 max_frames 的负数输入。")

    # 场景 2: 验证抽象基类 (ABC) 的强制实现约束 (对应文档 3.2)
    print("\n[测试 2] 验证接口抽象方法约束")
    class IncompleteValidator(IResourceValidator):
        # 故意不实现 can_execute 方法
        pass

    try:
        # 尝试实例化未实现抽象方法的子类
        bad_validator = IncompleteValidator()
    except TypeError as e:
        print(f" -> [成功] 捕获到 TypeError: {e}")
    else:
        print(" -> [失败] 允许实例化未完全实现的接口子类。")

    # 场景 3: 验证合规的接口实现
    print("\n[测试 3] 验证合规的接口子类实例化")
    class ValidValidator(IResourceValidator):
        # 按照规范实现抽象方法
        def can_execute(self, action_id: str, state) -> bool:
            return True
            
    try:
        good_validator = ValidValidator()
        print(" -> [成功] 合规子类实例化通过。")
    except Exception as e:
        print(f" -> [失败] 正确的实现抛出了异常 {e}")

    # 场景 4: 验证日志写入与自定义异常抛出 (对应文档 3.1 & 3.2)
    print("\n[测试 4] 验证日志写入与异常抛出")
    # 初始化日志系统
    setup_logging()
    logger = logging.getLogger("zsim.Core.Test")
    
    try:
        logger.info("准备抛出 ActionExecutionError 进行测试。")
        # 模拟动作校验失败
        raise ActionExecutionError("前置条件不足，动作无法释放。")
    except ActionExecutionError as e:
        logger.error(f"捕获到预期的自定义异常: {e}")
        print(" -> [成功] 自定义异常抛出并捕获。请前往 logs 文件夹检查生成的 log 文件以确认日志写入。")

if __name__ == "__main__":
    run_extended_verification()