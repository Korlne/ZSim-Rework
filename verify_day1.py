from core_control.game_state import GameState
from pydantic import ValidationError

def run_verification():
    print("开始验证 Day 1 交付基准...")
    try:
        # 尝试实例化带有非法负数帧数的状态对象
        state = GameState(current_frame=-1)
    except ValidationError as e:
        print("拦截成功！捕获到以下校验错误：")
        print(e)
    else:
        print("验证失败：未拦截负数输入。")

if __name__ == "__main__":
    run_verification()