# parallel/runner.py
"""
并行模拟运行器模块
主要功能：支持多线程并行执行不同队伍配置的模拟，每个线程独立游戏状态与 RNG。
使用 concurrent.futures.ThreadPoolExecutor 实现线程池调度。
"""
import logging
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field
from typing import List, Optional

logger = logging.getLogger("zsim.Parallel.Runner")


@dataclass
class SimConfig:
    """单次模拟的参数配置"""
    name: str = ""
    team_config_path: str = ""
    apl_path: str = ""
    seed: Optional[int] = None
    sim_mode: str = "FULL"        # LOOP / FULL / TIMED
    max_ticks: int = 18000
    max_duration_seconds: float = 300.0
    loop_count: int = 1


@dataclass
class SimResult:
    """单次模拟的结果"""
    config: SimConfig
    success: bool
    error_message: str = ""
    log_path: str = ""
    total_damage: float = 0.0


class ParallelRunner:
    """
    ParallelRunner 并行运行器
    主要功能：接收多个 SimConfig，在线程池中并发执行模拟，聚合结果。
    每个线程独立的 GameState、RNG 与日志文件。
    """

    def __init__(self, max_workers: int = None):
        self.max_workers = max_workers

    def run_parallel(self, configs: List[SimConfig]) -> List[SimResult]:
        """
        并行执行多个模拟配置。
        返回每个配置对应的 SimResult 列表。
        """
        results: List[SimResult] = []

        with ThreadPoolExecutor(max_workers=self.max_workers) as executor:
            futures = {
                executor.submit(self._run_single, cfg): cfg
                for cfg in configs
            }

            for future in as_completed(futures):
                cfg = futures[future]
                try:
                    result = future.result()
                    results.append(result)
                    logger.info(f"模拟完成: {cfg.name} (success={result.success})")
                except Exception as e:
                    logger.error(f"模拟失败: {cfg.name}: {e}")
                    results.append(SimResult(config=cfg, success=False, error_message=str(e)))

        return results

    @staticmethod
    def _run_single(config: SimConfig) -> SimResult:
        """
        执行单次模拟（独立线程内运行）。
        创建独立的 GameState、RNG、日志系统。
        """
        try:
            from core_control.game_state import GameState
            from core_control.event_bus import EventBus
            from calculation.rng_manager import RNGManager

            rng = RNGManager(seed=config.seed)
            state = GameState(
                max_ticks=config.max_ticks,
                current_tick=0,
                is_running=True,
            )

            # 简化版执行：此处仅做框架搭建，完整模拟需要加载数据并运行 EventBus
            # EventBus 的 run_simulation 将在 US-016 完成后驱动完整流程

            return SimResult(
                config=config,
                success=True,
                log_path=f"logs/simulation_{config.name}.jsonl",
            )
        except Exception as e:
            logger.error(f"模拟异常: {e}")
            return SimResult(config=config, success=False, error_message=str(e))
