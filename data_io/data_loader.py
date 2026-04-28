# io/data_loader.py
"""
数据加载器模块
主要功能：彻底剥离硬编码，从外部 JSON 文件加载角色面板、敌人数据、技能倍率、装备数据及 APL 排轴。
APL 格式参考 ZZZaxis_Timeline_example.json 的多轨道 tracks 结构，邦布与角色使用相同 schema。
"""
import json
import logging
from pathlib import Path
from typing import List, Dict, Optional

from pydantic import BaseModel, Field, ConfigDict

logger = logging.getLogger("zsim.IO.DataLoader")


class CharacterData(BaseModel):
    """角色导入数据结构"""
    model_config = ConfigDict(strict=True)

    char_id: str
    faction: str
    specialty: str
    element: str
    action_dict: List[str] = Field(default_factory=list)
    base_stats: Dict[str, float] = Field(default_factory=dict)


class EnemyData(BaseModel):
    """敌人导入数据结构"""
    model_config = ConfigDict(strict=True, populate_by_name=True)

    enemy_id: str
    enemy_type: str = "Normal"
    atk: float = 0.0
    hp: float = 0.0
    def_val: float = Field(default=0.0, alias="def")
    daze_max: float = 100.0
    res_physical: float = 0.0
    res_fire: float = 0.0
    res_ice: float = 0.0
    res_electric: float = 0.0
    res_ether: float = 0.0
    res_wind: float = 0.0


class DataLoader:
    """
    DataLoader 类
    主要功能：统一的外部数据加载入口，从指定目录或单文件加载各类游戏数据。
    支持多轨道 APL 排轴结构，邦布排轴与角色使用完全相同的 schema。
    """

    def __init__(self, data_root: str = "data"):
        self.data_root = Path(data_root)

    def _read_json(self, path: str) -> dict:
        with open(path, 'r', encoding='utf-8') as f:
            return json.load(f)

    def load_characters(self, json_path: Optional[str] = None) -> List[CharacterData]:
        """加载角色面板数据"""
        path = json_path or str(self.data_root / "characters" / "characters.json")
        data = self._read_json(path)
        # 兼容两种格式：列表或 {characters: [...]}
        items = data if isinstance(data, list) else data.get("characters", [])
        results = [CharacterData(**item) for item in items]
        logger.info(f"加载角色: {len(results)} 个")
        return results

    def load_enemies(self, json_path: Optional[str] = None) -> List[EnemyData]:
        """加载敌人数据"""
        path = json_path or str(self.data_root / "enemies" / "enemies.json")
        data = self._read_json(path)
        items = data if isinstance(data, list) else data.get("enemies", [])
        results = [EnemyData(**item) for item in items]
        logger.info(f"加载敌人: {len(results)} 个")
        return results

    def load_skill_data(self, json_path: Optional[str] = None) -> dict:
        """加载技能倍率数据，返回 {action_id: skill_data_dict}"""
        path = json_path or str(self.data_root / "skills" / "skills.json")
        data = self._read_json(path)
        items = data if isinstance(data, list) else data.get("skills", [])
        skill_map = {item["action_id"]: item for item in items}
        logger.info(f"加载技能数据: {len(skill_map)} 条")
        return skill_map

    def load_equipment(self, json_path: Optional[str] = None) -> dict:
        """
        加载装备数据，返回 {
            'wengines': [...], 'drive_discs': [...], 'disc_sets': [...]
        }
        """
        path = json_path or str(self.data_root / "equipment" / "equipment.json")
        return self._read_json(path)

    def load_apl_sequence(self, json_path: str) -> dict:
        """
        加载 APL 排轴 JSON，支持多轨道 tracks 结构。
        每个轨道对应一个角色/邦布，邦布排轴与角色使用完全相同的 schema。
        格式参考 Docs/ZZZaxis_Timeline_example.json。
        """
        data = self._read_json(json_path)
        scenario = data.get("scenarioList", [{}])[0]
        tracks = scenario.get("data", {}).get("tracks", [])
        logger.info(f"加载 APL: {len(tracks)} 条轨道")
        return {
            "scenario_id": scenario.get("id", ""),
            "tracks": tracks,
            "prep_duration": scenario.get("data", {}).get("prepDuration", 300),
        }
