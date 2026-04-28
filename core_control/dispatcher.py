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

# 战斗相关全局事件信号
on_damage_dealt = _zsim_signals.signal('on_damage_dealt')        # 伤害结算发出事件（结算前原始伤害）
on_damage_applied = _zsim_signals.signal('on_damage_applied')    # 伤害应用事件（经结算后最终伤害）
on_buff_changed = _zsim_signals.signal('on_buff_changed')        # BUFF变更事件（应用/叠层/移除）
on_anomaly_triggered = _zsim_signals.signal('on_anomaly_triggered')  # 属性异常触发事件
on_disorder_triggered = _zsim_signals.signal('on_disorder_triggered')  # 紊乱触发事件
on_chain_attack = _zsim_signals.signal('on_chain_attack')        # 连携技攻击事件
on_dodge = _zsim_signals.signal('on_dodge')                      # 闪避事件
on_parry = _zsim_signals.signal('on_parry')                      # 招架/弹刀事件
on_coordinated_action = _zsim_signals.signal('on_coordinated_action')  # 后台协同动作事件