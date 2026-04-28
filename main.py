"""
ZSim 2.0 全链路集成入口
主要功能：一键启动完整模拟流程：数据加载 → 队伍组建 → APL解析 → 逐帧模拟 → 日志输出 → 数据分析报表。
"""
from run_simulation import run_simulation


def main():
    """一键启动完整模拟"""
    log_path, report_path = run_simulation()
    print(f"\n模拟完成！")
    print(f"  日志: {log_path}")
    print(f"  报表: {report_path}")
    return log_path, report_path


if __name__ == "__main__":
    main()
