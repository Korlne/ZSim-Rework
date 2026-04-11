"""
集中式日志配置模块
主要功能：配置全局日志，将日志同步输出到控制台，
并自动在当前工作目录下创建 logs 文件夹，以系统时间动态命名日志文件。
"""
import logging.config
import os
from datetime import datetime

def setup_logging():
    """初始化并应用日志配置，动态生成带有时间戳的日志文件路径"""
    
    # 获取当前工作目录，并拼接 logs 文件夹路径
    log_dir = os.path.join(os.getcwd(), "logs")
    
    # 检查 logs 文件夹是否存在，如果不存在则自动创建
    os.makedirs(log_dir, exist_ok=True)
    
    # 获取当前系统时间，格式化为 年月日_时分秒 (例如：20260411_153000)
    current_time = datetime.now().strftime("%Y%m%d_%H%M%S")
    
    # 拼接最终的日志文件名和完整路径
    log_filename = f"{current_time}_debug.log"
    log_filepath = os.path.join(log_dir, log_filename)

    # 集中式日志配置字典
    LOGGING_CONFIG = {
        "version": 1,
        "disable_existing_loggers": False,
        "formatters": {
            "detailed": {
                # 定义包含时间、级别、模块名、行号与详细信息的日志格式
                "format": "%(asctime)s [%(levelname)s] | %(name)s | line:%(lineno)d | %(message)s",
                "datefmt": "%Y-%m-%d %H:%M:%S"
            }
        },
        "handlers": {
            "console": {
                "class": "logging.StreamHandler",
                "level": "DEBUG",
                "formatter": "detailed",
                "stream": "ext://sys.stdout"
            },
            "file": {
                "class": "logging.FileHandler",
                "level": "DEBUG",
                "formatter": "detailed",
                # 动态指定日志文件输出路径
                "filename": log_filepath,
                "mode": "w",
                "encoding": "utf8"
            }
        },
        "loggers": {
            "zsim.Core": {
                "level": "DEBUG",
                "handlers": ["console", "file"],
                "propagate": False
            }
        }
    }

    # 应用日志配置
    logging.config.dictConfig(LOGGING_CONFIG)