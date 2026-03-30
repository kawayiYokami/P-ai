---
name: news-analyst
description: 当用户要看今日热榜、平台热点、行业近况、突发事件、最新新闻、事实核验或某领域动态时，必须立刻阅读我。我提供“细分榜单优先 + 分类页兜底 + 多源校验 + 信息脱水 + 结构化输出”的统一流程。
---

# 新闻与热榜分析器

## 适用场景

以下需求默认命中本 skill：

- 今天有什么热点
- 今日热榜 / 今日新闻 / 今日大事
- 财经 / 科技 / AI / 开发 / 社区 / 综合新闻有什么值得看
- 最近发生了什么重要事情
- 帮我整理今天值得关注的内容
- 某个热点是真的假的
- 某个事件现在进展如何

## 工作模式

根据用户问题，优先选择最贴合的一种模式：

### 模式 A：热榜速览

适用于：
- 用户要“今天有哪些热点”
- 用户要快速看多个来源下最值得关注的内容
- 用户要扫一眼财经、科技、AI、开发、社区、综合等领域近况

### 模式 B：新闻核验 / 深度分析

适用于：
- 用户追问某个具体事件
- 用户要求验证真假
- 用户要求看最新进展、背景、影响

## 模式 A：热榜速览

### 默认策略

默认优先抓“细分榜单”，因为信噪比通常更高。

只有在以下情况，再补“分类页”：

- 细分榜单覆盖不够
- 用户要更全面的全景扫描
- 用户明确说“多找一点”“再扩展一下”“全网看看”

如果用户指定了领域，优先只抓与该领域直接相关的细分榜单，不要一上来铺太广。

### 第一层：细分榜单优先

优先从 `tophub.today` 抓取以下细分榜单：

- 百度：`https://tophub.today/n/Om4ejxvxEN`
- 虎扑：`https://tophub.today/n/G47o8weMmN`
- 知乎：`https://tophub.today/n/NaEdZ2ndrO`
- 36氪：`https://tophub.today/n/Q1Vd5Ko85R`
- 微博：`https://tophub.today/n/KqndgxeLl9`
- 微信：`https://tophub.today/n/WnBe01o371`
- 澎湃：`https://tophub.today/n/wWmoO5Rd4E`
- 头条：`https://tophub.today/n/x9ozB4KoXb`
- CCTV：`https://tophub.today/n/qndg1WxoLl`
- 新京报：`https://tophub.today/n/YqoXQ8XvOD`
- 观网：`https://tophub.today/n/RrvWOl3v5z`
- 华尔街见闻：`https://tophub.today/n/G2me3ndwjq`
- 新浪财经：`https://tophub.today/n/rx9ozj7oXb`
- 21财经：`https://tophub.today/n/4Kvx5R0dkx`
- 掘金：`https://tophub.today/n/rYqoXz8dOD`
- GitHub：`https://tophub.today/n/rYqoXQ8vOD`
- MIT Technology Review：`https://tophub.today/n/7GdabqLeQy`
- Google AI：`https://tophub.today/n/1Vd58gWv85`

推荐用法：

- 看社会/综合热点：百度、微博、知乎、微信、头条、澎湃、CCTV、新京报、观网
- 看财经：新浪财经、21财经、华尔街见闻、36氪
- 看开发：掘金、GitHub、知乎、36氪
- 看 AI：Google AI、MIT Technology Review、GitHub、36氪、知乎
- 看社区舆论：知乎、虎扑、微博、微信

### 第二层：分类页兜底

如果细分榜单不够，再补以下分类页：

- 财经：`https://tophub.today/c/finance`
- 开发：`https://tophub.today/c/developer`
- AI：`https://tophub.today/c/ai`
- 科技：`https://tophub.today/c/tech`
- 综合：`https://tophub.today/c/news`
- 社区：`https://tophub.today/c/community`

这些分类页里的内容通常一看便知，适合做扩展阅读、补盲区和查漏补缺，但一般不应先于高质量细分榜单。

### 第三层：可选高质量外部来源

当用户要求更专业、更前沿、或更国际化的信号时，可补充以下高质量站点：

- Hacker News：`https://news.ycombinator.com/`
- Lobsters：`https://lobste.rs/`
- Hugging Face Papers Trending：`https://huggingface.co/papers/trending`
- arXiv cs.AI：`https://arxiv.org/list/cs.AI/recent`
- arXiv cs.LG：`https://arxiv.org/list/cs.LG/recent`
- Papers with Code：`https://paperswithcode.com/`

这些来源适合补充开发、AI、研究前沿，不必每次默认抓取。

### 筛选准则

只保留真正有信息价值、且处于讨论焦点的话题：

1. 重大政策、国际关系、社会事件、公共安全、产业变化、技术进展、资本市场动态
2. 明显跨来源重复出现、讨论热度高、或具有现实影响的话题
3. 具备完整事件语义的标题，不能只剩半句、缩写、口号或抽象标签
4. 对用户指定领域有实际参考价值的资讯、公告、发布、事故、监管、市场异动、产品更新

默认降权或排除：

- 明星八卦
- 明显营销、广告、引流标题
- 只有情绪没有事实的主观评论
- 没有事实主体的口号式标题
- 重复、近似、价值很低的灌水内容

### 热榜输出

不要默认只总结 5 条。

原则是：只要有价值，就可以保留；如果有 8 条、12 条、15 条都值得看，就正常输出，不要为了凑“精简”而硬砍掉重要内容。

输出要求：

- 每条必须是完整事件句
- 只讲事实，不加评论
- 同类重复内容要合并去重
- 不强制附来源，除非用户要求，或来源本身对理解很关键
- 不要输出抓取过程、报错、工具细节

推荐格式：

```text
======

今日值得关注

1. ...
2. ...
3. ...
...

======
```

如果内容较多，可按领域分组，例如“财经 / 科技 / AI / 开发 / 社区 / 综合”。

## 模式 B：新闻核验 / 深度分析

### 核心准则

1. 时效性校验：默认优先看过去 24 小时内的进展，除非用户指定时间范围。
2. 信息脱水：剔除评论、站队、标题党表述，只保留事实。
3. 多源对齐：优先比对多个来源，提炼共识信息，并中立呈现冲突点。
4. 具体化：尽量保留时间、地点、人物、数字、结果。

### 执行流程

1. 先从最相关的细分榜单定位热点，再按需补分类页与真实新闻源。
2. 提取关键事实，按 5W1H 重组。
3. 若存在冲突，明确标注“已确认 / 待确认 / 不一致”。
4. 输出高密度总结，不写空话。

### 深度分析输出

推荐结构：

#### 一、核心结论

- 用一句话概括当前最新态势。

#### 二、关键事实

- 每条都要尽量带时间、主体、数据或动作。

#### 三、影响与背景

- 简述对行业、政策、市场、公众的直接影响。

#### 四、待确认点

- 明确哪些内容仍未证实。

#### 五、来源

- 列出核心来源链接。

## 通用要求

- 默认优先保留事实，不要写“炸了”“天塌了”“离谱”等情绪化措辞。
- 如果用户只是要“看热榜”，不要擅自展开成长篇评论。
- 如果用户要“核验”，不要只看热榜，必须补充真实新闻源或原始信息源。
- 如果某条热点明显只是观点、吐槽或评论，应主动降权或剔除。
- 如果多个来源都出现了同一事件，应主动合并表述，并把它视为高优先级线索。
