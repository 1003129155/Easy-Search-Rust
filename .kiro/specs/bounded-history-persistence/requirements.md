# 需求文档

## 简介

本特性收紧 EasySearch 使用历史的持久化边界：移除未接入的按查询固定项目功能，使频次表和最近项目列表保持明确容量上限；采用确定性淘汰规则优先保留近期且高频项目；兼容并归一化旧版 `history.json`；通过原子提交语义避免异常退出留下半写文件，并约束相关临时文件数量。

本次交付范围仅包含 `History` 及 `history.json`。同时，本规范建立项目级持久化容量原则，作为后续其他持续增长持久化数据的约束。

## 术语表

- **History**：EasySearch 中记录项目执行频次和最近执行项目的领域数据。
- **History_Store**：在内存中维护 `History`、记录执行事件并提供频次 boost 的组件。
- **History_Normalizer**：将加载数据或内存数据转换为满足容量、兼容性和排序约束的组件。
- **History_Persistence**：负责读取和提交 `%LOCALAPPDATA%\EasySearch\history.json` 的组件。
- **Project_Persistence_Policy**：约束项目中持续生成的持久化数据容量或轮转边界的项目级规则。
- **频次表**：`History.entries`，从 `action_key` 映射到无符号执行计数的集合。
- **最近列表**：`History.recent`，包含完整展示元数据并按执行先后维护的项目集合。
- **action_key**：唯一标识可执行搜索结果的字符串。
- **频次 boost**：依据执行计数增加搜索结果分数的现有分段映射，输出范围为 0 至 100。
- **近期项目**：`action_key` 当前存在于最近列表中的频次表项目。
- **旧版历史文件**：可能包含顶层 `pinned` 字段、超过 1000 个频次表项目或超过 20 个最近列表项目的可解析 `history.json`。
- **归一化**：删除废弃字段影响、去除重复最近项目并应用容量和淘汰规则，使 `History` 达到本规范定义的不变量。
- **保留顺序**：用于选择频次表保留项目的确定性优先级：近期项目优先；近期状态相同时执行计数较高者优先；前两项相同时按 `action_key` Unicode 标量值升序优先。
- **完整历史文件**：能够被 `History_Persistence` 完整读取并解析为一个 JSON 文档的 `history.json`。
- **提交**：使新版本完整历史文件取代上一个已提交版本的持久化操作。
- **异常退出**：进程在历史文件提交期间被终止或操作系统中断写入的情况。
- **持续生成的持久化数据**：随应用运行持续新增或累积、且不会仅由固定配置规模决定的数据。

## 需求

### 需求 1：移除按查询固定项目功能

**用户故事：** 作为维护者，我希望删除未接入且不需要的按查询固定项目能力，以便缩小 `History` 数据模型和维护范围。

#### 验收标准

1. THE History_Store SHALL 不提供按查询固定、取消固定、固定状态查询或固定位置查询能力。
2. THE History_Persistence SHALL 在新写入的 `history.json` 中省略 `pinned` 字段。
3. WHEN History_Persistence 读取包含 `pinned` 字段的旧版历史文件，THE History_Persistence SHALL 忽略 `pinned` 字段并继续加载其余受支持字段。

### 需求 2：限制 History 容量

**用户故事：** 作为用户，我希望历史数据具有固定上限，以便长期使用不会导致 `history.json` 无界增长。

#### 验收标准

1. THE History_Store SHALL 将频次表限制为最多 1000 个不同的 `action_key`。
2. THE History_Store SHALL 将最近列表限制为最多 20 个不同的 `action_key`。
3. WHEN History_Store 记录已存在的 `action_key`，THE History_Store SHALL 使用饱和加法将对应执行计数增加 1。
4. WHEN History_Store 记录项目完整元数据，THE History_Store SHALL 将对应项目置于最近列表最新位置并删除最近列表中相同 `action_key` 的旧项目。
5. WHEN History_Store 记录项目后任一容量超过上限，THE History_Normalizer SHALL 在记录操作返回前将频次表和最近列表缩减至各自上限。
6. WHEN History_Persistence 提交 `History`，THE History_Persistence SHALL 仅提交包含最多 1000 个频次表项目和最多 20 个最近列表项目的归一化数据。

### 需求 3：确定性淘汰与排序行为

**用户故事：** 作为用户，我希望淘汰策略优先保存近期且高频的项目，以便有限历史容量继续改善常用项目排序。

#### 验收标准

1. WHEN History_Normalizer 从频次表选择最多 1000 个保留项目，THE History_Normalizer SHALL 严格按照保留顺序选择优先级最高的项目。
2. WHEN 两次归一化接收具有相同频次表内容和相同近期项目集合的 `History`，THE History_Normalizer SHALL 产生相同的频次表键值集合。
3. WHEN 新 `action_key` 使已满频次表达到 1001 个项目，THE History_Normalizer SHALL 保留新 `action_key` 并淘汰保留顺序最低的其他项目。
4. WHEN History_Normalizer 从最近列表中删除超出 20 条的项目，THE History_Normalizer SHALL 删除最早执行的项目并保留最新的 20 个不同 `action_key`。

### 需求 4：保持现有 boost 行为

**用户故事：** 作为用户，我希望容量治理不改变已有频次排序增益，以便搜索结果排序体验保持稳定。

#### 验收标准

1. WHEN `action_key` 的执行计数为 0，THE History_Store SHALL 返回 0 的频次 boost。
2. WHEN `action_key` 的执行计数为 1 至 2，THE History_Store SHALL 返回 20 的频次 boost。
3. WHEN `action_key` 的执行计数为 3 至 9，THE History_Store SHALL 返回 40 的频次 boost。
4. WHEN `action_key` 的执行计数为 10 至 29，THE History_Store SHALL 返回 60 的频次 boost。
5. WHEN `action_key` 的执行计数为 30 至 99，THE History_Store SHALL 返回 80 的频次 boost。
6. WHEN `action_key` 的执行计数大于或等于 100，THE History_Store SHALL 返回 100 的频次 boost。

### 需求 5：兼容并归一化旧版历史文件

**用户故事：** 作为升级用户，我希望 EasySearch 可读取旧历史数据并自动收敛到新边界，以便升级后无需手工删除历史文件。

#### 验收标准

1. WHEN History_Persistence 读取可解析的旧版历史文件，THE History_Normalizer SHALL 在加载操作返回前归一化 `History`。
2. WHEN 旧版历史文件的频次表超过 1000 个项目，THE History_Normalizer SHALL 按保留顺序保留优先级最高的 1000 个项目。
3. WHEN 旧版历史文件的最近列表超过 20 个不同 `action_key`，THE History_Normalizer SHALL 保留最新的 20 个不同 `action_key`。
4. WHEN 旧版历史文件的最近列表包含重复 `action_key`，THE History_Normalizer SHALL 保留每个 `action_key` 最后出现的项目并维持保留项目的相对时间顺序。
5. IF `history.json` 不存在、不可读取或不是可解析的受支持 JSON 文档，THEN THE History_Persistence SHALL 返回空的归一化 `History`。
6. WHEN 归一化后的 `History` 再次提交，THE History_Persistence SHALL 写入不含 `pinned` 字段且满足全部容量上限的当前格式历史文件。

### 需求 6：异常退出安全且文件数量有界的持久化

**用户故事：** 作为用户，我希望历史保存不会因异常退出破坏已提交数据，也不会持续遗留临时文件，以便历史持久化长期可靠且磁盘占用有界。

#### 验收标准

1. WHEN History_Persistence 成功提交 `History`，THE History_Persistence SHALL 使 `history.json` 表示本次提交的完整历史文件。
2. IF 提交期间发生异常退出，THEN THE History_Persistence SHALL 使下次加载可获得本次提交前或本次提交后的完整历史文件。
3. THE History_Persistence SHALL 将历史持久化相关文件限制为一个 `history.json` 和最多一个具有固定名称的临时文件。
4. WHEN History_Persistence 开始加载或提交，THE History_Persistence SHALL 清理不参与当前提交的固定名称临时文件。
5. IF 临时文件写入或提交替换失败，THEN THE History_Persistence SHALL 保留上一个已提交的完整 `history.json` 并以非致命失败结束本次保存。
6. WHEN History_Persistence 提交历史文件，THE History_Persistence SHALL 在与 `history.json` 相同的目录中完成临时版本写入和文件替换。

### 需求 7：项目级持久化容量原则

**用户故事：** 作为维护者，我希望持续生成的持久化数据都有明确容量或轮转上限，以便项目中的磁盘使用保持可预测。

#### 验收标准

1. THE Project_Persistence_Policy SHALL 要求每一种持续生成的持久化数据具有可验证的容量上限或轮转上限。
2. THE Project_Persistence_Policy SHALL 要求每一种持续生成的持久化数据定义达到上限时的确定性保留或删除规则。
3. WHERE 本次特性范围适用，THE Project_Persistence_Policy SHALL 仅将 `History` 和 `history.json` 纳入实现与验证范围。
4. WHEN 后续特性新增持续生成的持久化数据，THE Project_Persistence_Policy SHALL 要求对应特性规范声明容量或轮转上限及达到上限时的行为。
