# Merge Worktree Orchestration Scripts

## Overview

这套合并脚本用于有序、可恢复的合并 500+ 个 git worktree 分支，采用类似归并排序的并行策略。25 个子代理同时处理每个包含约 20 个分支的分组，然后将结果按两级向上合并。

## Files

### Primary Orchestration

- **`merge_orchestrator.sh`** - 主控脚本，负责协调整个合并过程
  - 备份、分析、L0 并行处理、L1 配对合并、L2+ 批量合并、验证、报告

- **`merge_worker.sh`** - 并发工作脚本，处理单个 bucket 的分支合并
  - 支持 worker/pair-merge/batch-merge 三种模式

### Supporting Files

- **`merge-manifest.json`** - 测试用的合并清单
  - 包含 2 个 bucket × 2 个分支的示例

- **`merge-test.sh`** - 脚本验证套件
  - 语法检查、依赖检查、功能验证

### Documentation

- **`MERGE_STATE_MACHINE.md`** - 状态机详解
  - 各阶段操作、缓存处理、恢复流程

- **`MERGE_USAGE.md`** - 使用指南
  - 基本用法、以后恢复、冲突解决、CI/CD 集成

## 快速开始

```bash
# 1. 验证脚本
./scripts/merge-test.sh

# 2. 启动合并管理器
MAX_PARALLELISM=25 \
./scripts/merge_orchestrator.sh \
  ./scripts/merge-manifest.json \
  /path/to/worktree/base \
  main

# 3. 检查日志
cat .merge-state/L0-results.json
```

## 工作流程

### 阶段说明

1. **BACKUP** - 创建完整备份包
   ```bash
   git bundle create full-backup.bundle --all
   ```

2. **ANALYSIS** - 解析并分析合并清单
   - 验证 JSON 结构
   - 统计分支数量

3. **L0-BUCKET** - 并行处理分组
   - 25 个 worker 同时工作
   - 每个分组约 20 个分支
   - 合并成功 → 记录 commit hash
   - 合并冲突 → 隔离到 quarantined 列表

4. **L1-PAIR** - 配对合并结果
   - 将 L0 结果两两配对
   - 逐步合并，直到剩下一个结果

5. **L2..LN** - 批量合并
   - 按 MAX_PARALLELISM 分批处理
   - 直到最终单个分支

6. **VERIFY** - 验证最后结果
   ```bash
   cargo check
   ```

7. **REPORT** - 生成报告
   - 合并统计
   - 隔离分支列表
   - 成功/失败状态

### 状态跟踪

- 锁文件：`state/bucket-*.*.lock` (并发保护)
- 进程文件：`state/bucket-*.*.pid` (恢复追踪)
- 结果文件：`state/LN-results.json`

### 恢复机制

- Git 标签：`merge-LN-complete` (阶段标记)
- 检测停机位置，跳过已完成阶段
- 保留错误工作树供手动修复

## 参数配置

- `MAX_PARALLELISM` - 并发数（默认 25）
- `WORKTREE_BASE` - 工作树基础目录
- `BASE_BRANCH` - 基础分支（通常 main）

```bash
# 自定义并发数
MAX_PARALLELISM=50 ./scripts/merge_orchestrator.sh [args]
```

## 错误处理

- 使用 `set -euo pipefail` 确保失败时立即退出
- 冲突分支隔离到 quarantined 列表
- 详细冲突报告（文件名、冲突次数、来源）
- 保留有冲突的工作树用于手动修复

## 安全与限制

- 磁盘空间：至少 50GB
- 并发数：根据系统资源调整
- 锁文件防止重复合并
- 自动清理成功的工作树
- 手动确认重启前的工作树状态

## 输出格式

### Worker 结果

```json
{
  "bucketIndex": "0",
  "status": "SUCCESS | QUARANTINED",
  "mergeCommit": "abc123...",
  "quarantined": [
    {
      "branch": "feature/topology-add-edge",
      "conflicts": 3,
      "files": "src/topology/mod.rs, src/graph/mod.rs",
      "source": "upstream/opencode-v2-feature-slice",
      "base": "abc123def456"
    }
  ],
  "startTime": "2026-10-01T00:00:00Z",
  "endTime": "2026-10-01T00:30:00Z",
  "branchesProcessed": 20,
  "branchesTotal": 20,
  "branchesQuarantined": 2
}
```

## 集成

### GitLab CI

```yaml
merge_worktrees:
  before_script:
    - apk add --no-cache jq git
    - chmod +x scripts/merge_*.sh
  script:
    - |
      ./scripts/merge_orchestrator.sh \
        ./merge-manifest.json \
        /ci/worktree-base \
        main
```

### GitHub Actions

```yaml
- name: Start merge orchestration
  env:
    MAX_PARALLELISM: 25
  run: |
    ./merge_orchestrator.sh \
      ./merge-manifest.json \
      /Users/mymac/Projects/opencode-rk-v2-integration \
      main
```

## 故障排查

- **Orphaned workers**：`pkill -f merge_worker.sh`
- **Stale lock files**：`find . -name "*.lock" -delete`
- **Orphaned worktrees**：`git worktree prune`
- **Manual cleanup**：`rm -rf .merge-state .merge-wtrees`

## 依赖

- `jq` - JSON 解析
- `git` - 版本控制
- `make` - Rust 项目构建（验证阶段）

```bash
brew install jq git
```

## 注意事项

- 使用前确保 worktree 有足够磁盘空间
- 在并发数高时监控 CPU/内存使用
- 首次运行建议从小规模清单测试
- 重要操作前先创建备份
- 冲突分支可在处理后手动解决

## 完整示例

```bash
# 测试脚本
./scripts/merge-test.sh

# 小规模测试（2 bucket × 2 branches）
MAX_PARALLELISM=2 \
./scripts/merge_orchestrator.sh \
  ./scripts/merge-manifest.json \
  ~/test-worktree \
  main

# 生产运行（500+ branches）
MAX_PARALLELISM=25 \
./merge-orchestrator.sh \
  /path/to/merge-manifest.json \
  /path/to/v2-integration \
  main
```

详见 `MERGE_USAGE.md`。