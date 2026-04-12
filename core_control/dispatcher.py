"""
全局事件与信号分发模块
主要功能：定义并管理系统中所有的全局事件信号，实现基于发布-订阅(Pub-Sub)模式的模块解耦。
"""
from blinker import Namespace

# 创建 zsim 专属的信号命名空间
_zsim_signals = Namespace()

# 定义核心全局事件信号
on_tick = _zsim_signals.signal('on_tick')                      # 帧驱动事件，每帧向中间层广播
on_action_request = _zsim_signals.signal('on_action_request')  # 动作请求执行事件
on_action_start = _zsim_signals.signal('on_action_start')      # 动作开始执行事件
on_error_raised = _zsim_signals.signal('on_error_raised')      # 异常发生与捕获事件
on_combat_end = _zsim_signals.signal('on_combat_end')          # 战斗模拟结束事件