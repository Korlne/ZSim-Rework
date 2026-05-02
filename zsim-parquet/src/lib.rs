// zsim-parquet: ZSim 2.0 模拟结果的列式存储与聚合。
// 处理使用 Zstd 压缩的 Parquet 写入和基于 Arrow 的聚合查询。

pub mod aggregator;
pub mod writer;
