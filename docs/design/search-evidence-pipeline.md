# Search Evidence Pipeline: Schema Delivery State Machine

Status: **Jason 已批准，进入实现**

Design ID: `search-evidence-schema-delivery-pipeline-20260815-v2`

Date: 2026-08-15

## 1. Goal

Jason 要求的搜索交付不是“搜索后直接总结”，也不是靠 prompt 固定流程，而是：

1. 搜索流程的每个阶段都交付 typed schema。
2. 只有 schema validator 和状态机允许的阶段转换才能进入下一阶段。
3. prompt 只负责产生下一份 delivery schema；prompt 文本不是执行 gate。
4. 每个交付信源必须有真实 URL。
5. 每个 URL 必须被实际访问。
6. 访问后必须保留正文或结构化证据。
7. 每条事实必须绑定已访问且通过验证的信源。
8. 信源不可访问、内容不支持结论、内容过期或信源冲突时，显式标记，不能包装成已验证事实。
9. 只有信源验证完成后才允许最终总结。
10. 第一发现通道优先使用 provider-hosted `web_search`，但 hosted search 结果必须包含原始链接；只有摘要没有 URL 的结果不能进入候选信源。
11. hosted search 返回的每个候选 URL 都必须使用 camo 实际访问和验证；没有 camo typed verification delivery 的链接不能支撑最终结论。
12. camo 独立社交搜索用于 hosted search 结果不满意、覆盖不足、需要补充或需要更多信源时；目标平台包括小红书、微博、X。
13. 搜索规划按领域优先选择高权重信源；新闻优先补充微博，操作类和教程优先补充小红书。

## 1a. GitHub 参考审计

### 审计范围

Jason 要求先查 GitHub 上 star 较高的 agent schema / skill 设计项目，再决定是否吸收。本设计已下载并在 `/tmp/freehand-agent-schema-research*` 完成源码级核对，未安装任何外部 skill；以下仓库按 GitHub API 当前数据记录。

| 仓库 | star | license | 审计 commit | 价值 |
| --- | --- | --- | --- | --- |
| `anthropics/skills` | 169511 | Anthropic 自有条款（仓库无 SPDX 字段） | `f6656c1256d5a8adfa37db9110046ef20bac644c` | 官方 skill-creator：断言必须可验证、必须带 evidence；不写不可判定的“看起来不错”；有/无 skill 基线对比；只保留有区分度的断言 |
| `agentskills/agentskills` | 24301 | Apache-2.0 | `69ef37e9424c0a7ea9dd2293b559e43ec8176379` | 机器可读 skill 规范：name 格式、description 长度、frontmatter 严格校验；提供 skills-ref 校验器 |
| `vamplabAI/sgr-agent-core` | 1114 | MIT | `a41efdefff2d0ae57cbb19108c59cf080ca89ab2` | 每个阶段用 discriminated union 收窄可用工具；达到迭代上限后只剩 report/final 工具 |
| `sno-ai/mda` | 615 | Apache-2.0 | `03a0f4126bf1ca0f9428618865c9b9dd65fada40` | fixture 必须绑定 rule id 和 expected verdict；valid/invalid/compile 三类语料；CI 必跑 conformance suite |
| `microsoft/skills-for-copilot-studio` | 407 | MIT | `f7b65888c47e9b3f18c050b15f20cd8dd500b2c5` | eval prompt 不点名 skill/schema；断言必须校验正确 skill 被调用；用 `stdout_not_contains` 拦截 general-knowledge fallback 和 raw execution 错误 |
| `anivar/zod-skill` | 20 | MIT | `bb0620d90ed0b6f693f24f6941ade54c9bc7d330` | strict object 拒未知字段；discriminated union 走 tag；错误输出结构化嵌套路径；schema 显式版本；按 schema/field 出指标 |
| `bbmosquito/forgiving-contracts` | 3 | MIT | `fd29828225aee56ec5b5217fe75569f0427a7059` | 语义字段名、具体示例、跨模型实验和可读错误消息可用；但 string→array、null→optional、自动 repair 与 Freehand 的 strict/no-fallback 冲突，明确不采用 |

### 已采纳约束

1. 阶段工具暴露必须是 discriminated union / stage-specific tool set，不能把全量工具一直暴露：
   - `domain_planning`：无搜索工具。
   - `hosted_discovery`：只暴露 provider-hosted `web_search`。
   - `verification / social_discovery`：只暴露 `camo`，不暴露普通 `web_fetch` 作为 verified 通道。
   - 达到 schema retry 上限后，禁止探索工具继续产出；只能进入显式 blocked 或重试收口。
2. 每份 delivery schema 必须带显式 `schema` 版本和 tagged discriminator，Rust 端用 `#[serde(deny_unknown_fields)]` + enum 分支拒绝未知字段和错误阶段。
3. schema rejection 必须给出机器可读字段路径；不能只报 `freehand_search_delivery` 整体失败。至少输出到顶层字段；嵌套数组项失败必须带 `candidates[i].field` 风格路径。
4. 增加机器可读 conformance 语料：valid/invalid fixture 每条绑定 rule id、expected verdict、error category；CI/构建链必须执行该语料，不能只靠人工阅读。
5. 增加自然语言 eval：prompt 不出现 `SearchDiscoveryDelivery`、`camo`、`weibo` 等 schema/tool 名字；断言只验证正确 sourced-search 流程被进入、正确阶段工具被使用、错误信源被拒绝，并反向断言没有 general-knowledge fallback、没有直接总结、没有 raw execution 错误泄漏。
6. 指标只按 schema/stage/field 聚合 rejection 次数；不记录原始 prompt、provider payload、工具参数或 cookie 内容。
7. skill/schema 的 eval 必须双向比较：
   - 有 sourced-search schema/状态机 vs 无 schema 基线；
   - 每次 PASS 必须带具体证据（delivery 字段、source-id 解析结果、阶段序列），不能写“输出看起来不错”。

## 2. 当前已有状况

### 已存在能力

- Provider-hosted `web_search` 已接入 OpenAI Responses 和 Anthropic Messages：
  - `provider.semantic` 只把 `web_search_call` 作为 reasoning observation，没有结构化 URL/正文/访问状态清单。
  - `clean_search` Worker 执行 profile 已存在，只使用 provider-hosted search，不暴露本地函数工具。
- `tool.registry` 已有：
  - `web_fetch`：实际抓取 URL 并返回有界正文文本。
  - `camo`：已实现 Camoufox 0.4.2 CLI 封装，支持 profile、cookie、`search`、`fetch-page`、`get-readable`。
  - `camo search` 当前本机安装版本帮助中只确认 `xhs`；微博和 X 平台均未由本机 CLI 能力证据确认。
- 完成契约：
  - `CompletionSubmission` 有 `claim/summary/evidence/learned/completion_reason`。
  - `evidence` 是自由文本，没有 typed sources，也没有“结论 -> 信源 -> 证据片段”的机器可查关系。
- 工具结果契约：
  - `ToolExecutionOutput` 只有 `text`。
  - `ToolResultContract` 只有 `output` 字符串。
  - 因此搜索结果的 URL、访问状态、正文证据只能由模型从文本里再组织，框架无法校验。

### 当前流程为什么不够

1. provider search 的 URL 和摘要只进入模型文本，模型可以省略 URL、改写 URL 或直接总结。
2. 没有“访问过 URL”的机器事实，最终 summary 无法与真实抓取结果绑定。
3. `clean_search` 禁止本地工具，所以即使搜索结果里有 URL，也不能在同一 search turn 用 `web_fetch` 或 camo 访问。
4. camo 虽然已有 cookie 能力，但当前 `clean_search` 不暴露它；社交平台搜索没有进入正式搜索流程。
5. 完成 schema 没有 typed source delivery，所以“无信源总结”可以合法通过。
6. 当前设计把流程约束放在 prompt 里；prompt 可以漏执行，不能作为交付 gate。

## 3. Schema-First Target Architecture

### 执行原则

- 搜索交付约束是 typed delivery schema + validator + state machine，不是 prompt。
- 每个阶段模型或工具 owner 必须交付一份带 `schema` 和 `delivery_id` 的 typed delivery。
- 缺少字段、未知字段、字段类型错误、阶段转换越界，都进入 typed rejection/retry/blocked 链。
- 任何失败都显式暴露；禁止 fallback、silent strip、用记忆补 URL、用模型文本冒充工具访问结果。
- prompt 只说明当前阶段期望交付哪份 schema、字段语义和候选规则；validator 决定是否允许进入下一阶段。
- `search_evidence` 是业务交付事实，不是控制面 metadata；访问时间、访问状态、正文证据和结论引用都是业务 payload。
- `ToolResultContract` / camo tool owner 输出扩展为结构化 delivery payload，不再只传 `text`；provider/tool 输出进入 `reason.turn` 后才成为可持久化、可投影的 `search_evidence` 真源。

### 新增业务资源：`search_evidence`

在 `docs/resource-maps/core.json` 新增 `search_evidence`，owner 为 `reason.turn`：

- `provider_hosted_search.project_candidate`：把 hosted `web_search` 投影成 `SearchDiscoveryDelivery`，并强制原始 URL。
- `tool_call.verify_search_url`：对候选 URL 使用 camo profile 实际访问，输出 `SearchVerificationDelivery`。
- `tool_call.discover_social_candidate`：hosted 不足时通过 camo profile/cookie 输出社交 `SearchDiscoveryDelivery`。
- `search_evidence.rank_domain_sources`：根据 typed domain plan 对候选排序，保留领域分类和权重依据。
- `search_evidence.apply_final_delivery`：校验 claim/source-id 引用并写入 `SearchEvidenceTurnDelivery`。
- `search_evidence.project_to_ui`：把 turn 真源投影成 UI 证据。

`search_evidence` 是业务 payload，不是控制面 metadata；hosted discovery、camo verification、claim binding 和 UI 投影都由这个资源链约束。

### Delivery Schema 总表

| 阶段 | 生产者 | Delivery Schema | Validator / Parser |
| --- | --- | --- | --- |
| 领域规划 | 模型 | `SearchDomainPlanDelivery` | `validate_search_domain_plan_delivery` |
| hosted 候选发现 | provider semantic / adapter owner | `SearchDiscoveryDelivery`（`discovery_channel=hosted_web_search`） | `validate_search_discovery_delivery` |
| camo 社交候选发现 | camo tool owner | `SearchDiscoveryDelivery`（`discovery_channel=camo_social_search`） | `parse_camo_search_delivery` + `validate_search_discovery_delivery` |
| URL 页面验证 | camo tool owner | `SearchVerificationDelivery` | `parse_camo_verification_delivery` + `validate_search_verification_delivery` |
| 社交补充决策 | 模型 | `SocialSupplementDecisionDelivery` | `validate_social_supplement_decision_delivery` |
| 最终交付 | 模型 | `SearchFinalDelivery` | `validate_search_final_delivery` |
| turn 证据真源 | `reason.turn` | `SearchEvidenceTurnDelivery` | `validate_search_evidence_turn_delivery` |
| UI 投影 | `ui.protocol` | `UiSearchEvidenceProjection` | DTO validator / projection validator |

共享 schema 类型放 `freehand-contracts`，纯校验和状态转换放 `freehand-blocks`。所有 delivery 都是可序列化、可重放、可持久化的业务契约。

### 1. `SearchDomainPlanDelivery`

```json
{
  "schema": "search_evidence.domain_plan.v1",
  "delivery_id": "domain-20260815-001",
  "domain": "news",
  "preferred_source_kinds": [
    "official_publication",
    "mainstream_news",
    "eyewitness_account"
  ],
  "social_platform_priority": ["weibo", "x"],
  "minimum_verified_sources": 2,
  "policy_version": "2026-08-15"
}
```

字段约束：

- `domain`：`news | tutorial | operations | technical | policy | local_review | general`。
- `preferred_source_kinds`：非空字符串数组。
- `social_platform_priority`：按领域策略的固定候选顺序；`news` 必须是 `weibo` 优先，`tutorial/operations` 必须是 `xhs` 优先。
- `minimum_verified_sources`：正整数，默认至少 `1`。
- 缺少 `policy_version`、`domain` 或 `minimum_verified_sources` 时 schema rejection，不进入 discovery。

### 2. `SearchDiscoveryDelivery`

```json
{
  "schema": "search_evidence.discovery.v1",
  "delivery_id": "discovery-hosted-001",
  "discovery_channel": "hosted_web_search",
  "domain_plan_ref": "domain-20260815-001",
  "hosted_search_attempt": {
    "query": "示例查询",
    "provider": "openai_responses"
  },
  "candidates": [
    {
      "candidate_id": "c1",
      "status": "usable",
      "original_url": "https://example.com/post/123",
      "title": "示例帖子标题",
      "snippet": "搜索摘要，不能作为 verified 证据",
      "discovered_by": "hosted_web_search",
      "platform": "weibo",
      "source_weight": 90
    },
    {
      "candidate_id": "c2",
      "status": "unusable_missing_url",
      "original_url": null,
      "title": "没有原始链接的结果",
      "snippet": "只有摘要",
      "reason": "hosted_search_did_not_return_original_url"
    }
  ]
}
```

字段约束：

- `discovery_channel`：`hosted_web_search | camo_social_search`。
- `hosted_search_attempt`：仅 `discovery_channel=hosted_web_search` 时必填；camo 社交发现不携带 provider hosted attempt。
- `candidates[].original_url`：`status=usable` 时必填且必须是非空 http/https URL。
- `status`：`usable | unusable_missing_url | unusable_other`。
- 缺少 URL 的候选只能显式标为 `unusable`；不能进入 verification。
- `discovered_by`：`hosted_web_search | camo_social_search`。
- `platform`：`web | xhs | weibo | x | other`。
- `source_weight`：只决定验证顺序和补充优先级，不替代证据验证。
- 候选数组为空或全部 unusable 时，不能进入 final；只能补充或 blocked。
- provider adapter 不能输出模型自造的 URL；如果 provider 不提供原始 URL，候选必须标 `unusable_missing_url`。

### 3. `SearchVerificationDelivery`

由 camo tool owner 从结构化执行结果生成，模型不能手工填写：

```json
{
  "schema": "search_evidence.verification.v1",
  "delivery_id": "verify-c1-001",
  "source_id": "c1",
  "original_url": "https://example.com/post/123",
  "camo_profile": "weibo",
  "accessed_at": "2026-08-15T12:00:00Z",
  "access_status": "verified",
  "page_title": "示例帖子标题",
  "evidence_excerpt": "页面正文中与结论直接相关的原文片段",
  "verified_by": "camo",
  "access_attempts": [
    {
      "attempt_id": "a1",
      "channel": "camo",
      "status": "http_error",
      "accessed_at": "2026-08-15T11:59:00Z",
      "error": {"code": "http_403", "message": "..."}
    },
    {
      "attempt_id": "a2",
      "channel": "camo",
      "status": "verified",
      "accessed_at": "2026-08-15T12:00:00Z",
      "error": null
    }
  ],
  "error": null
}
```

失败示例：

```json
{
  "schema": "search_evidence.verification.v1",
  "delivery_id": "verify-c2-001",
  "source_id": "c2",
  "original_url": "https://example.com/blocked",
  "camo_profile": "weibo",
  "accessed_at": "2026-08-15T12:01:00Z",
  "access_status": "blocked",
  "page_title": null,
  "evidence_excerpt": null,
  "verified_by": null,
  "access_attempts": [
    {
      "attempt_id": "a1",
      "channel": "camo",
      "status": "blocked",
      "accessed_at": "2026-08-15T12:01:00Z",
      "error": {"code": "login_required", "message": "..."}
    }
  ],
  "error": {"code": "login_required", "message": "..."}
}
```

字段约束：

- `access_status`：`verified | http_error | timeout | blocked | not_accessed`。
- `verified` 只允许在 camo 实际访问成功且正文存在非空 `evidence_excerpt` 时出现。
- `verified_by` 只能是 `camo`；hosted search observation、模型文本、`web_fetch` 都不能满足 verified。
- `access_attempts` 按真实执行顺序记录，不能只保留最后一次成功。
- 失败/冲突/过期/无法访问时显式保留 `error`，不能静默删除。

### 4. `SocialSupplementDecisionDelivery`

```json
{
  "schema": "search_evidence.supplement_decision.v1",
  "delivery_id": "supplement-001",
  "domain_plan_ref": "domain-20260815-001",
  "required": true,
  "reasons": ["insufficient_verified_sources"],
  "platforms": ["weibo"]
}
```

字段约束：

- `required=false` 时，`reasons` 和 `platforms` 可为空。
- `required=true` 时，`reasons` 至少一项，`platforms` 至少一项。
- `reasons` enum：
  - `missing_original_urls`
  - `insufficient_verified_sources`
  - `low_weight_coverage`
  - `single_source_only`
  - `source_conflict`
  - `insufficient_evidence`
  - `user_requested_more_sources`
  - `user_requested_social_source`
- 新闻必须优先 `weibo`；教程/操作类必须优先 `xhs`；用户明确要求 X 时平台列表可包含 `x`。
- validator 同样按领域检查 `platforms`：新闻缺少 `weibo`、教程/操作缺少 `xhs` 时 rejection。
- 平台不支持时显式失败，禁止静默跳过或 fallback 到普通 web search。

### 5. `SearchFinalDelivery`

模型提交引用，不提交新的 URL/证据事实：

```json
{
  "schema": "search_evidence.final.v1",
  "delivery_id": "final-001",
  "domain_plan_ref": "domain-20260815-001",
  "claim": "complete",
  "summary": "最终总结",
  "claims": [
    {
      "claim_id": "k1",
      "text": "结论 1",
      "source_ids": ["c1", "c3"]
    }
  ],
  "unconfirmed": [
    {
      "source_id": "c2",
      "reason": "page_evidence_conflicts"
    }
  ],
  "blocked_reason": null
}
```

blocked 示例：

```json
{
  "schema": "search_evidence.final.v1",
  "delivery_id": "final-002",
  "domain_plan_ref": "domain-20260815-001",
  "claim": "blocked",
  "summary": null,
  "claims": [],
  "unconfirmed": [
    {
      "source_id": "c1",
      "reason": "page_could_not_be_opened"
    }
  ],
  "blocked_reason": "no_verified_source"
}
```

字段约束：

- `claim=complete` 时必须有非空 `summary`，且每个 claim 至少引用一个已验证 source。
- `claim=blocked` 时必须有 `blocked_reason`，不能生成正常总结。
- `source_ids` 只能引用同一 turn 已持久化的 `SearchVerificationDelivery`；validator 用 source id 解析真实 URL、访问状态和证据，不能接受模型新写出的 URL。
- 未验证、访问失败、冲突、过期的 source 只能进入 `unconfirmed`，不能支撑 claim。

### 6. `SearchEvidenceTurnDelivery`

`reason.turn` 在阶段 delivery 全部通过后构建 turn 真源：

```json
{
  "schema": "search_evidence.turn.v1",
  "turn_id": "runtime-turn-42",
  "domain_plan": {...},
  "deliveries": [...],
  "verified_sources": [...],
  "unconfirmed": [...],
  "claims": [...],
  "status": "final_validated",
  "summary_ready": true,
  "terminal": "success"
}
```

- 只允许由 `reason.turn` 写入并持久化。
- `summary_ready` 只能来自 `FinalDeliveryValidated` 状态。
- `verified_sources` 是从 camo typed verification delivery 解析出的 owner truth，不是模型自报。
- `CompletionSubmission` 不复制 `sources`；如需要终态关联，只引用 `search_delivery_id`。

### 状态机

```text
SearchDomainPlanValidated
  -> HostedDiscoveryValidated
  -> CamoVerificationRequired
  -> CamoVerificationValidated
  -> SupplementDecisionValidated
  -> [SocialDiscoveryValidated -> CamoVerificationValidated]*
  -> FinalDeliveryValidated
  -> TurnTerminalSuccess
```

只允许相邻转换：

- 不允许从 `HostedDiscoveryValidated` 直接进入 `FinalDeliveryValidated`。
- 不允许把 hosted candidate 直接标成 verified。
- `HostedDiscoveryValidated` 有可用候选时，必须先进入 `CamoVerificationRequired`。
- `HostedDiscoveryValidated` 无可用候选时，直接进入 `SupplementDecisionValidated`；不能以空验证集进入 final。
- `SupplementDecisionValidated` 中 `required=false` 时直接进入 `FinalDeliveryValidated`；`required=true` 时必须先进入 `SocialDiscoveryValidated`。
- 每个社交发现候选仍必须回到 `CamoVerificationValidated`。
- `FinalDeliveryValidated` 之前不允许 `summary_ready=true`。

任何 schema parse、validator 或状态转换失败进入 typed error/retry 链：

- 缺 schema、缺 `delivery_id`、未知字段、类型错误、枚举值非法：`SearchEvidenceRejected`，返回当前阶段和期望 schema。
- 验证失败但可补充：允许进入补充决策。
- 没有已验证信源：只允许 blocked，禁止 complete。
- retry 达到上限仍失败：显式 failed，禁止写 Success。

## 4. Prompt 与 Schema 的边界

### Prompt 不是 gate

prompt 只负责：

1. 告诉模型当前状态期望交付哪份 delivery schema。
2. 说明每个字段的语义。
3. 说明领域权重、hosted-first、camo-verify、社交平台优先级等策略。
4. 说明失败时必须显式输出失败状态，不能编造 URL 或证据。

模型通过 `<freehand_search_delivery>` tagged JSON block 提交下一份 delivery；hosted provider adapter 和 camo tool owner 通过 typed tool result / semantic event 提交自己的 delivery。任何路径缺失 schema 或字段非法都由 validator 拒绝。

决定流程是否继续、是否 verified、是否 complete 的只有：

- typed schema parse/validation；
- state machine 的相邻转换规则；
- camo tool owner 的结构化访问结果；
- `reason.turn` 的最终 delivery validator。

### `sourced_search` 的 schema 生产要求

- 先判定领域并交付 `SearchDomainPlanDelivery`。
- 第一阶段优先交付 `SearchDiscoveryDelivery`，`discovery_channel=hosted_web_search`。
- hosted 每个候选必须包含原始 URL；没有 URL 的结果显式标 `unusable_missing_url`。
- hosted snippet 一律不是 verified；之后必须产生 camo verification delivery。
- 对每个可用候选 URL 使用 camo 实际访问；验证结果必须是 `SearchVerificationDelivery`。
- hosted 结果不足、单一来源、冲突、低覆盖、用户要求更多来源时，交付 `SocialSupplementDecisionDelivery`。
- 社交补充产生的候选再次交付 `SearchDiscoveryDelivery`，并再次 camo 验证。
- 最终交付 `SearchFinalDelivery`，validator 解析 source id 到已验证的 camo delivery。

### 完成契约

- `sourced_search` 终态先通过 `SearchFinalDelivery` validator，再进入通用 `CompletionSubmission`。
- `CompletionSubmission` 不再承载 `sources` 真源；可增加可选 `search_delivery_id` 引用。
- 模型 `evidence` 自由文本永远不能替代 `SearchVerificationDelivery.evidence_excerpt`。
- 完成校验由 `reason.turn` / `freehand-blocks` 执行，不能由模型自证。

## 5. 领域权重与社交搜索策略

领域分类在搜索开始前产生 typed `SearchDomainPlanDelivery`：

- `domain`：查询领域。
- `preferred_source_kinds`：优先信源类型。
- `social_platform_priority`：需要补充时的平台顺序。
- `minimum_verified_sources`：该查询至少需要的已验证信源数。
- `policy_version`：策略版本。

初始策略：

| 领域 | 高权重信源 | 社交补充优先级 |
| --- | --- | --- |
| 新闻/突发事件 | 官方发布、当事机构、主流新闻机构、现场账号 | 微博优先，必要时 X |
| 操作方法/教程 | 官方文档、产品帮助、真实操作经验、步骤完整内容 | 小红书优先 |
| 技术问题 | 官方文档、源码/issue、维护者说明、可复现技术文章 | X 只作维护者动态补充 |
| 政策/规则 | 政府、监管机构、官方公告、原始文件 | 社交平台只作线索，不替代正式文件 |
| 消费/本地体验 | 官方信息、专业评测、真实用户体验 | 小红书优先，必要时微博 |

- 权重决定查询顺序和候选排序，不是“高权重即真实”。所有来源仍要经过 camo 访问和正文证据校验。
- 领域策略属于 typed policy/plan，不写进 provider metadata，也不允许 WebUI 自己推断。
- `news` 领域必须由 validator 拒绝 `social_platform_priority` 中微博缺失的 plan。
- `tutorial/operations` 领域必须由 validator 拒绝 `social_platform_priority` 中小红书缺失的 plan。

## 6. Camo Social Search 平台策略

- cookie 通过 `~/.camo/profiles/<platform>/` 持久化；模型只传 profile 名，永远不把 cookie 内容放入 prompt/log/tool result。
- hosted-first、camo-verify 规则对所有搜索生效：hosted search 先发现原始链接，camo 逐条验证；camo 独立搜索只在需要补充时启动。
- `camo search` 仅对本机 CLI 明确声明支持的平台输出 typed `SearchDiscoveryDelivery`；当前只确认 `xhs`。
- 微博和 X 在本机能力未确认前必须返回 typed unsupported-platform 错误，不能按 skill 旧说明或模型知识宣称可用。
- X 的当前安装版本未确认支持：设计不写自动 fallback，而是把“X 搜索”明确绑定到两种实现之一：
  1. camo CLI 增加 `x` platform adapter；或
  2. 在 `tool.registry` 中实现显式 `x_search` 命令，使用 profile 打开 `x.com/search?q=...` 再 `get-readable`。
- 平台不支持时必须显式失败，不能静默跳过、不能用普通 web search 顶替社交搜索。

## 7. 失败矩阵

| 失败 | 允许状态 | 禁止行为 |
| --- | --- | --- |
| hosted 结果缺原始 URL | `unusable_missing_url`，可补充或 blocked | 不能进入 verification，不能支撑 claim |
| hosted 无任何可用候选 | 社交补充或 blocked | 不能 final complete |
| camo 打不开 URL | `blocked/http_error/timeout`，进入 unconfirmed | 不能标 verified，不能凭记忆补正文 |
| 页面正文无证据 | `not_verified`，进入 unconfirmed | 不能支撑 claim |
| 来源冲突/过期 | 保留冲突/过期状态，进入 unconfirmed | 不能静默丢弃或按 verified 使用 |
| verified source 数量不足 | 触发补充；无法补充时 blocked | 不能 final complete |
| schema 缺字段/未知字段/类型错误 | `SearchEvidenceRejected` / retry | 不能写 Success |
| 状态转换越界 | rejection + 期望状态 | 不能跳过 camo 验证或提前 final |
| provider/tool 输出格式错误 | 显式 schema 错误，回 owner 修 | 不能靠模型重新解释通过 |
| 无 verified source 的总结 | blocked | 禁止 complete |

## 8. WebUI 完整流程

搜索不新建独立结果页，直接作为当前会话 turn 的完整生命周期渲染。WebUI 只消费 typed `UiSearchEvidenceProjection`，不解析 camo/provider 原始输出。

### 运行中

1. 用户消息立即显示在当前 turn。
2. assistant 区域先显示搜索进度，不提前显示结论：
   - `正在通过网络搜索发现信源`
   - `正在用浏览器验证链接`
   - `正在补充微博信源`或`正在补充小红书信源`
   - `正在访问 N 个信源`
   - `已验证 X / 失败 Y / 冲突 Z`
3. 每个信源实时渲染一行紧凑状态：
   - 平台、标题、域名/作者、真实 URL。
   - 当前步骤：`网络搜索发现`、`浏览器验证`、`微博补充搜索`、`小红书补充搜索`、`已验证`、`无法验证`、`内容冲突`。
   - `access_attempts` 按顺序显示，不能只展示最后一次成功。
4. 推理未完成时只显示信源进度和证据状态，不展示没有 source binding 的总结。

### 完成后

1. 最终总结在信源区域之后渲染。
2. 每条结论带稳定引用编号，例如 `[S1] [S3]`；点击引用滚动到对应信源。
3. 每张信源卡显示：
   - 标题、平台、URL、访问时间、`verified_by`。
   - 支持该结论的正文证据片段。
   - `在新窗口打开`链接，直接访问真实 URL。
4. 未验证、访问失败、内容过期和冲突项进入单独的`未确认项`区域，不能混进已验证结论。
5. 移动端使用单列紧凑证据行；桌面端允许展开证据详情，但不嵌套卡片。

### Protocol projection

`UiTurnProjection` 增加可选 `search_evidence`：

- `stage`: `domain_planning | discovering | accessing | supplementing | verifying | summarizing | complete | blocked`。
- `domain_plan`: `UiSearchDomainPlanProjection`，显示领域标签、最低信源要求和使用的来源策略。
- `sources`: `UiSearchSourceProjection[]`，从 owner `SearchEvidenceTurnDelivery` 投影。
- `claims`: `UiSearchClaimProjection[]`，每条 claim 只引用 verified `source_id`。
- `unconfirmed`: `UiSearchUnconfirmedProjection[]`。
- `summary_ready`: 只有 `FinalDeliveryValidated` 且 `reason.turn` 持久化 `SearchEvidenceTurnDelivery` 后为 `true`。
- `blocked_reason`: 由 final validator 投影，不允许 UI 推断。

ADP 通过现有 selected-session turn query/subscribe 增量推送这些字段。浏览器只保存展开/收起状态，不缓存或重建信源真相。

## 9. Owner / 文件边界

| 改动 | owner feature | 涉及文件 |
| --- | --- | --- |
| `search_evidence` resource + relation | `reason.turn` / resource map | `docs/resource-maps/core.json`, `crates/freehand-reason` |
| typed delivery schema | `contracts.core` | `crates/freehand-contracts` |
| delivery validator + state machine | `freehand-blocks` / `reason.turn` | `crates/freehand-blocks`, `crates/freehand-reason` |
| hosted 候选 URL 语义投影 | `provider.semantic` + adapter owners | `crates/freehand-provider-core`, `crates/freehand-provider-openai`, `crates/freehand-provider-anthropic` |
| 领域分类/信源权重纯规则 | `reason.turn` / `freehand-blocks` | `crates/freehand-reason`, `crates/freehand-blocks` |
| camo typed search/verification delivery | `tool.registry` | `crates/freehand-tools` |
| `sourced_search` profile + schema 状态编排 | `runtime.master-worker-loop` | `crates/freehand-runtime` |
| 完成契约校验 | `reason.turn` / `freehand-blocks` | `crates/freehand-reason`, `crates/freehand-blocks` |
| UI 信源证据投影 | `ui.protocol` / `app.webui-smoke` | `crates/freehand-ui-protocol`, `apps/freehand-server` |
| 生命周期 manifest | `reason.turn` / `runtime.master-worker-loop` | `docs/lifecycles/search-evidence-delivery.json` |
| 架构/function/test/mainline/wiki 同步 | 每个 owner 各自更新 | `docs/function-maps`, `docs/testing`, `docs/mainline-calls`, `docs/wiki` |

禁止越过 owner：

- Android 只消费协议投影，不拥有搜索逻辑。
- WebUI 只消费 typed `search_evidence` projection，不解析 provider 原始 payload。
- `provider.semantic` 不新增本地搜索执行。
- `tool.registry` 不拥有 reason 完成校验。
- `CompletionSubmission` 不承载 search evidence 真源。

## 10. 验证方案

### 本地测试

- 每个 delivery schema round-trip + 严格 rejection 测试：缺字段、未知字段、类型错误、非法 enum。
- 状态机正反测试：
  - 正向：DomainPlan -> HostedDiscovery -> CamoVerification -> SupplementDecision(false) -> Final -> Terminal。
  - 正向：DomainPlan -> HostedDiscovery -> CamoVerification -> SupplementDecision(true) -> SocialDiscovery -> CamoVerification -> Final -> Terminal。
  - 反向：HostedDiscovery 直接 Final 必须拒绝。
  - 反向：无 camo verification 的 source 必须拒绝。
  - 反向：hosted-only 候选标 verified 必须拒绝。
- hosted `web_search` 先执行且每个可用候选都包含原始 URL；缺 URL 候选标 unusable。
- camo typed verification 是唯一 verified 来源；`web_fetch`、模型文本、hosted snippet 都不能通过。
- 新闻查询缺少微博补充优先级时 plan validator 拒绝；教程/操作缺少小红书时拒绝。
- camo argv/schema 测试覆盖 `xhs`、`weibo`、`x` 命令。
- Worker `sourced_search` 使用本地 HTTP fixture + camo profile fixture 完成：发现 -> 访问 -> 证据 -> 补充决策 -> 最终交付。
- 最终 delivery 的 `source_ids` 必须解析到同一 turn 的 camo verification delivery；模型新写 URL/evidence 会被拒绝。
- 无 verified source 时只允许 blocked，禁止 complete。

### 在线验证

- S-profile daemon 安装重启后跑真实 `sourced_search` 任务。
- 真实 camo profile 验证小红书、微博；X 视 camo 平台支持进度单独验证。
- 验证最终 transcript 中每个 source 都有 URL、访问状态、访问时间、证据片段。
- 验证 hosted 发现 -> camo 逐链接验证 -> 领域社交补充 -> 最终总结的完整顺序。
- 验证访问失败时模型不能凭记忆补全。
- WebUI 验证信源证据卡片来自 owner projection，不是摘要猜测。
- WebUI 验证运行中不提前总结、完成后引用可跳转、失败/冲突项与已验证结论分离。

## 11. 实施阶段

1. 阶段 1（已完成）：Jason 批准 design id `search-evidence-schema-delivery-pipeline-20260815-v2`。
2. 阶段 2（当前）：resource map、lifecycle manifest、contracts、blocks validator、test design 先行。
3. 阶段 3：hosted result URL projection、camo typed verification output、领域权重规则。
4. 阶段 4：`sourced_search` profile、schema 状态机编排、社交补充触发器。
5. 阶段 5：WebUI 增量信源状态、证据卡、引用跳转和未确认项投影。
6. 阶段 6：本地 gates + daemon 在线 + 真实社交平台验证，再走 codex review。

## 12. 已批准约束

- design id `search-evidence-schema-delivery-pipeline-20260815-v2` 已批准。
- 每阶段 delivery schema + validator + state machine 是唯一执行 gate；prompt 只负责产出下一份 schema。
- hosted-first/camo-verify 为主流程；hosted 可用结果必须带原始链接。
- `SearchFinalDelivery` 只提交 claim/source-id 引用；URL 和证据由 `reason.turn` 从 camo verification delivery 解析。
- 缺 URL、打不开、无证据、冲突、数量不足、schema 错误、越界转换均不能进入 complete。
- 新闻优先微博，操作/教程优先小红书；权重只决定搜索顺序，不替代验证。
- 微博/X 当前能力缺口不授权 fallback；未确认支持时显式阻塞，后续平台 adapter 需独立设计和在线证据。
