# kiro-rs

一个用 Rust 编写的 Anthropic Claude API 兼容代理服务，将 Anthropic API 请求转换为 Kiro API 请求。

---

<table>
<tr>
<td>
<b>推荐</b>：<a href="https://kissapi.ai">KissAPI</a> 一站式 AI API 中转服务，支持 Claude、GPT、Gemini 等主流模型，OpenAI 兼容端点，稳定高可用。<br>
如您有意体验, 请点击链接注册体验 → <a href="https://kissapi.ai">立即访问</a>
</td>
</tr>
</table>

---

#### [LINUX DO 讨论帖](https://linux.do/t/topic/1571986)

## 免责声明

本项目仅供研究使用, Use at your own risk, 使用本项目所导致的任何后果由使用人承担, 与本项目无关。
本项目与 AWS/KIRO/Anthropic/Claude 等官方无关, 本项目不代表官方立场。

## 注意！

因 TLS 默认从 native-tls 切换至 rustls，你可能需要专门安装证书后才能配置 HTTP 代理。可通过 `config.json` 的 `tlsBackend` 切回 `native-tls`。
如果遇到请求报错, 尤其是无法刷新 token, 或者是直接返回 error request, 请尝试切换 tls 后端为 `native-tls`, 一般即可解决。

> `native-tls` 只在默认 feature 构建（`cargo build`）中可用。Docker 镜像和 musl 发布件使用 `--no-default-features` 编译，只包含 rustls，配置 `native-tls` 会在启动时报错。

**Write Failed/会话卡死**: 如果遇到持续的 Write File / Write Failed 并导致会话不可用，参考 Issue [#22](https://github.com/hank9999/kiro.rs/issues/22) 和 [#49](https://github.com/hank9999/kiro.rs/issues/49) 的说明与临时解决方案（通常与输出过长被截断有关，可尝试调低输出相关 token 上限）

## 功能特性

- **Anthropic API 兼容**: 完整支持 Anthropic Claude API 格式
- **流式响应**: 支持 SSE (Server-Sent Events) 流式输出；`/v1` 流式带 90 秒上游静默看门狗
- **Token 自动刷新**: 支持 `social`、`idc`（IdC / Builder ID / IAM）、`external_idp`（企业 SSO）三种刷新方式，以及无需刷新的 Kiro API Key 凭据
- **多凭据支持**: 按 `priority`（优先级）或 `balanced`（均衡）选择凭据，额度耗尽 / 凭据失效自动禁用，风控 429 自动冷却
- **凭据级限流**: 滑动窗口限流规则，支持全局默认与凭据级覆盖
- **凭据回写**: 多凭据格式下自动回写刷新后的 Token 与禁用状态
- **Thinking 模式**: 支持 extended thinking（`enabled` / `adaptive`）与 effort 等级，4.6 及更新的模型走原生 reasoning 字段
- **工具调用**: 完整支持 function calling / tool use
- **WebSearch**: 内置 WebSearch 工具转换逻辑
- **图片与文档**: 入站图片自动降采样到上游限制内，PDF / 文本文档自动抽取文字
- **多模型支持**: Sonnet 4.5 / 4.6 / 5、Opus 4.5 / 4.6 / 4.7 / 4.8 / 5、Haiku 4.5、GPT-5.6
- **多端点**: 可按凭据选择 `aws` / `ide` / `cli` 三种 Kiro 上游端点
- **Admin 管理**: 可选的 Web 管理界面和 API，支持凭据管理、余额查询、代理池、运行时设置热更新
- **多级 Region 配置**: 支持全局和凭据级别的 Auth Region / API Region 配置
- **凭据级代理**: 支持为每个凭据单独配置 HTTP/SOCKS5 代理，优先级：凭据代理 > 全局代理 > 无代理

---

- [开始](#开始)
  - [1. 编译](#1-编译)
  - [2. 最小配置](#2-最小配置)
  - [3. 启动](#3-启动)
  - [4. 验证](#4-验证)
  - [Docker](#docker)
- [配置详解](#配置详解)
  - [config.json](#configjson)
  - [credentials.json](#credentialsjson)
  - [Region 配置](#region-配置)
  - [代理配置](#代理配置)
  - [限流配置](#限流配置)
  - [端点配置](#端点配置)
  - [认证方式](#认证方式)
  - [环境变量](#环境变量)
  - [运行时文件](#运行时文件)
- [API 端点](#api-端点)
  - [标准端点 (/v1)](#标准端点-v1)
  - [Claude Code 兼容端点 (/cc/v1)](#claude-code-兼容端点-ccv1)
  - [Thinking 模式](#thinking-模式)
  - [工具调用](#工具调用)
  - [图片与文档](#图片与文档)
- [模型映射](#模型映射)
- [多凭据与故障转移](#多凭据与故障转移)
- [Admin（可选）](#admin可选)
- [注意事项](#注意事项)
- [项目结构](#项目结构)
- [技术栈](#技术栈)
- [License](#license)
- [致谢](#致谢)

## 开始

### 1. 编译

> PS: 如果不想编译可以直接前往 Release 下载二进制文件

> **前置步骤**：编译前需要先构建前端 Admin UI（用于嵌入到二进制中）：
> ```bash
> cd admin-ui && pnpm install && pnpm build
> ```
> 如果不需要 Admin UI，也至少要保证 `admin-ui/dist` 目录存在（空目录即可），否则 `rust-embed` 会在编译期报错。

```bash
cargo build --release
```

musl 目标与 Docker 镜像使用 `cargo build --release --no-default-features`（不包含 native-tls）。

### 2. 最小配置

创建 `config.json`：

```json
{
   "host": "127.0.0.1",
   "port": 8990,
   "apiKey": "sk-kiro-rs-qazWSXedcRFV123456",
   "region": "us-east-1"
}
```
> PS: 如果你需要 Web 管理面板, 请注意配置 `adminApiKey`

创建 `credentials.json`（从 Kiro IDE 等中获取凭证信息）：
> PS: 可以前往 Web 管理面板配置跳过本步骤
> 如果你对凭据地域有疑惑, 请查看 [Region 配置](#region-配置)

Social 认证：
```json
{
   "refreshToken": "你的刷新token",
   "expiresAt": "2025-12-31T02:32:45.144Z",
   "authMethod": "social"
}
```

IdC 认证：
```json
{
   "refreshToken": "你的刷新token",
   "expiresAt": "2025-12-31T02:32:45.144Z",
   "authMethod": "idc",
   "clientId": "你的clientId",
   "clientSecret": "你的clientSecret"
}
```

### 3. 启动

```bash
./target/release/kiro-rs
```

或指定配置文件路径：

```bash
./target/release/kiro-rs -c /path/to/config.json --credentials /path/to/credentials.json
```

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `-c, --config <PATH>` | `config.json` | 配置文件路径 |
| `--credentials <PATH>` | `credentials.json` | 凭据文件路径 |

### 4. 验证

```bash
curl http://127.0.0.1:8990/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: sk-kiro-rs-qazWSXedcRFV123456" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 1024,
    "stream": true,
    "messages": [
      {"role": "user", "content": "Hello, Claude!"}
    ]
  }'
```

### Docker

也可以通过 Docker 启动：

```bash
docker-compose up
```

`docker-compose.yml` 默认使用镜像 `ghcr.io/hank9999/kiro-rs:latest`（可通过 `IMAGE_OWNER` / `IMAGE_TAG` 环境变量覆盖），并把宿主机 `./config/` 目录挂载到容器 `/app/config/`。请把 `config.json` 和 `credentials.json` 放在该目录下，运行时产生的状态文件（见 [运行时文件](#运行时文件)）也会写在这里。

容器内请把 `host` 设为 `0.0.0.0`，否则端口映射后无法从宿主机访问。镜像使用 `--no-default-features` 编译，`tlsBackend` 只能是 `rustls`。

## 配置详解

### config.json

| 字段 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `host` | string | `127.0.0.1` | 服务监听地址 |
| `port` | number | `8080` | 服务监听端口 |
| `apiKey` | string | - | 自定义 API Key（用于客户端认证，必配） |
| `region` | string | `us-east-1` | AWS 区域 |
| `authRegion` | string | - | Auth Region（用于 Token 刷新），未配置时回退到 region |
| `apiRegion` | string | - | API Region（用于 API 请求），未配置时回退到 region |
| `kiroVersion` | string | `0.11.107` | Kiro 版本号（拼入 User-Agent） |
| `machineId` | string | - | 全局机器码，接受 64 位十六进制或 UUID。未配置时按凭据确定性派生（API Key 凭据取 `sha256("KiroAPIKey/<kiroApiKey>")`，OAuth 凭据取 `sha256("KotlinNativeAPI/<refreshToken>")`），并回写到凭据文件 |
| `systemVersion` | string | 按 machineId 固定 | 系统版本标识。未显式配置时从 10 个候选值中按 machineId 确定性选取，重启不变 |
| `nodeVersion` | string | `22.22.0` | Node.js 版本标识 |
| `tlsBackend` | string | `rustls` | TLS 后端：`rustls` 或 `native-tls`（后者仅默认 feature 构建可用） |
| `countTokensApiUrl` | string | - | 外部 count_tokens API 地址；未配置或调用失败时回退本地估算 |
| `countTokensApiKey` | string | - | 外部 count_tokens API 密钥 |
| `countTokensAuthType` | string | `x-api-key` | 外部 API 认证类型：`x-api-key` 或 `bearer` |
| `proxyUrl` | string | - | HTTP/SOCKS5 代理地址 |
| `proxyUsername` | string | - | 代理用户名 |
| `proxyPassword` | string | - | 代理密码 |
| `adminApiKey` | string | - | Admin API 密钥，配置后启用凭据管理 API 和 Web 管理界面；空白字符串视为未配置 |
| `loadBalancingMode` | string | `priority` | 负载均衡模式：`priority`（按优先级）或 `balanced`（均衡分配），可在 Admin 中热改 |
| `suspiciousCooldownSeconds` | number | `600` | Kiro 上游 suspicious activity 429 触发后的运行时冷却秒数（Admin UI 运行时设置中以分钟展示，可即时修改）。配置为 `0` 时仍按 600 秒冷却 |
| `defaultRateLimits` | array | - | 全局默认限流规则，见 [限流配置](#限流配置) |
| `nativeLikeTwoPhaseFlow` | boolean | `false` | 实验特性：每次请求先用 `simple-task` 模型发送一次去掉工具的预飞请求，再发主请求（模拟 Kiro IDE 行为），可在 Admin 中热改 |
| `extractThinking` | boolean | `true` | 非流式响应的 thinking 块提取。启用后 `<thinking>` 标签会被解析为独立的 `thinking` 内容块，可在 Admin 中热改 |
| `defaultEndpoint` | string | `aws` | 默认 Kiro 端点。凭据未显式指定 `endpoint` 时使用。当前支持：`aws` / `ide` / `cli`，见 [端点配置](#端点配置) |
| `endpoints` | object | `{}` | 预留字段（按端点名存放参数对象），当前代码没有读取它 |

说明：
- 配置文件不存在时使用全部默认值启动，但 `apiKey` 缺失会直接退出。
- Admin 中热改的四项（负载均衡模式、冷却时间、`extractThinking`、`nativeLikeTwoPhaseFlow`）会重新读取并整体重写 `config.json`，文件中的注释和未知字段会丢失，带默认值的字段会被显式写出。

完整配置示例：

```json
{
   "host": "127.0.0.1",
   "port": 8990,
   "apiKey": "sk-kiro-rs-qazWSXedcRFV123456",
   "region": "us-east-1",
   "tlsBackend": "rustls",
   "kiroVersion": "0.11.107",
   "machineId": "64位十六进制机器码",
   "systemVersion": "darwin#24.6.0",
   "nodeVersion": "22.22.0",
   "authRegion": "us-east-1",
   "apiRegion": "us-east-1",
   "countTokensApiUrl": "https://api.example.com/v1/messages/count_tokens",
   "countTokensApiKey": "sk-your-count-tokens-api-key",
   "countTokensAuthType": "x-api-key",
   "proxyUrl": "http://127.0.0.1:7890",
   "proxyUsername": "user",
   "proxyPassword": "pass",
   "adminApiKey": "sk-admin-your-secret-key",
   "loadBalancingMode": "priority",
   "suspiciousCooldownSeconds": 600,
   "extractThinking": true,
   "nativeLikeTwoPhaseFlow": false,
   "defaultEndpoint": "aws",
   "defaultRateLimits": [
      { "window": "1m", "maxRequests": 30 }
   ]
}
```

### credentials.json

支持单对象格式（向后兼容）或数组格式（多凭据）。文件不存在或为空时按零凭据启动（可稍后通过 Admin 添加）。

#### 字段说明

| 字段 | 类型 | 描述 |
|------|------|------|
| `id` | number | 凭据唯一 ID（可选，仅用于 Admin API 管理；手写文件可不填） |
| `accessToken` | string | OAuth 访问令牌（可选，可自动刷新） |
| `refreshToken` | string | OAuth 刷新令牌 |
| `profileArn` | string | AWS Profile ARN（可选）。缺失时 OAuth / IdC 凭据会在刷新后自动调用 `ListAvailableProfiles` 解析并回写，仍解析不到则填入内置默认值 |
| `expiresAt` | string | Token 过期时间 (RFC3339)。距过期 10 分钟内会触发刷新，无法解析视为已过期 |
| `authMethod` | string | 认证方式：`social` / `idc` / `external_idp` / `api_key`，见下文 |
| `provider` | string | 身份提供商标识（KAM 导出字段，如 `BuilderId` / `Enterprise` / `Github` / `Google` / `IAM_SSO`），用于缺少 `profileArn` 时辅助判断 |
| `clientId` | string | IdC / External IdP 登录的客户端 ID |
| `clientSecret` | string | IdC 登录的客户端密钥（IdC 认证必填） |
| `tokenEndpoint` | string | External IdP 的 token endpoint（`external_idp` 必填） |
| `issuerUrl` | string | External IdP issuer URL（可选，仅保留） |
| `scopes` | string | External IdP OAuth scopes（可选，刷新时作为 `scope` 参数带上） |
| `kiroApiKey` | string | Kiro API Key（`ksk_` 开头）。设置后直接作为 Bearer Token 使用，不需要 `refreshToken`，也不参与刷新 |
| `priority` | number | 凭据优先级，数字越小越优先，默认为 0 |
| `region` | string | 凭据级 Auth Region，兼容字段；**不**参与 API Region 的回退链 |
| `authRegion` | string | 凭据级 Auth Region，用于 Token 刷新，未配置时回退到 `region` |
| `apiRegion` | string | 凭据级 API Region，用于 API 请求 |
| `machineId` | string | 凭据级机器码（64 位十六进制或 UUID） |
| `email` | string | 用户邮箱（可选，刷新时自动识别并回写） |
| `displayName` | string | 自定义显示名称（可选，仅用于 Admin UI 展示；也接受 `display_name`） |
| `subscriptionTitle` | string | 订阅等级（如 `KIRO PRO+` / `KIRO FREE`，查询余额时自动更新）。含 `FREE` 的凭据不会被分配 Opus 请求 |
| `proxyUrl` | string | 凭据级代理 URL（可选，特殊值 `direct` 表示不使用代理） |
| `proxyUsername` | string | 凭据级代理用户名（可选） |
| `proxyPassword` | string | 凭据级代理密码（可选） |
| `disabled` | boolean | 是否禁用，默认 `false`。运行时被自动禁用后回写文件时会同步此字段 |
| `endpoint` | string | 凭据级端点名称（可选，`aws` / `ide` / `cli`，未配置时使用 `config.defaultEndpoint`；填了未注册的名字会导致启动失败） |
| `allowOverage` | boolean | 是否允许超额使用，默认 `false`。开启后本地额度判定放宽 10000，上游明确返回额度耗尽时仍会禁用 |
| `rateLimits` | array | 凭据级限流规则，存在时整体覆盖 `config.defaultRateLimits`，见 [限流配置](#限流配置) |

`authMethod` 说明（大小写不敏感）：
- `social`：Kiro 社交登录，刷新走 `prod.<authRegion>.auth.desktop.kiro.dev`
- `idc`：IdC / Builder ID / IAM 在本项目里属于同一种登录方式，刷新走 `oidc.<authRegion>.amazonaws.com`，需要 `clientId` + `clientSecret`。为兼容旧配置，`builder-id` / `iam` 仍可被识别，加载时按 `idc` 处理
- `external_idp`：企业 SSO（如 M365 / Entra ID），刷新直接请求凭据自带的 `tokenEndpoint`，需要 `clientId` + `tokenEndpoint`
- `api_key`（别名 `apikey`）：Kiro API Key 凭据，只需要 `kiroApiKey`
- 未填写时：有 `clientId` + `clientSecret` 按 `idc` 处理，否则按 `social`

#### 单凭据格式（旧格式，向后兼容）

```json
{
   "accessToken": "请求token，一般有效期一小时，可选",
   "refreshToken": "刷新token，一般有效期7-30天不等",
   "profileArn": "arn:aws:codewhisperer:us-east-1:111112222233:profile/QWER1QAZSDFGH",
   "expiresAt": "2025-12-31T02:32:45.144Z",
   "authMethod": "social",
   "clientId": "IdC 登录需要",
   "clientSecret": "IdC 登录需要"
}
```

#### External IdP 凭据

```json
{
   "refreshToken": "你的刷新token",
   "expiresAt": "2025-12-31T02:32:45.144Z",
   "authMethod": "external_idp",
   "clientId": "你的clientId",
   "tokenEndpoint": "https://<你的 IdP>/oauth2/v2.0/token",
   "scopes": "openid profile offline_access"
}
```

#### Kiro API Key 凭据

```json
{
   "kiroApiKey": "ksk_your_api_key_here",
   "authMethod": "api_key"
}
```

也可以通过环境变量 `KIRO_API_KEY` 注入一条最高优先级的 API Key 凭据，见 [环境变量](#环境变量)。

#### 多凭据格式（支持故障转移和自动回写）

```json
[
   {
      "refreshToken": "第一个凭据的刷新token",
      "expiresAt": "2025-12-31T02:32:45.144Z",
      "authMethod": "social",
      "priority": 0,
      "endpoint": "ide"
   },
   {
      "refreshToken": "第二个凭据的刷新token",
      "expiresAt": "2025-12-31T02:32:45.144Z",
      "authMethod": "idc",
      "clientId": "xxxxxxxxx",
      "clientSecret": "xxxxxxxxx",
      "region": "us-east-2",
      "priority": 1,
      "proxyUrl": "socks5://proxy.example.com:1080",
      "proxyUsername": "user",
      "proxyPassword": "pass",
      "rateLimits": [
         { "window": "5m", "maxRequests": 50 }
      ]
   },
   {
      "refreshToken": "第三个凭据（显式不走代理）",
      "expiresAt": "2025-12-31T02:32:45.144Z",
      "authMethod": "social",
      "priority": 2,
      "proxyUrl": "direct"
   }
]
```

多凭据特性：
- 按 `priority` 字段排序，数字越小优先级越高（默认为 0）
- Token 刷新成功、Admin 修改凭据、启动时补齐 `id` / `machineId` 时会整体回写源文件（写入时同步当前 `disabled` 状态、归一化 `authMethod`）；运行时自动禁用本身不触发写入；单对象格式不回写
- 凭据选择与故障转移的具体行为见 [多凭据与故障转移](#多凭据与故障转移)

更多示例见仓库内 `credentials.example.social.json`、`credentials.example.idc.json`、`credentials.example.apikey.json`、`credentials.example.multiple.json`。

### Region 配置

支持多级 Region 配置，分别控制 Token 刷新和 API 请求使用的区域。

**Auth Region**（Token 刷新）优先级：
`凭据.authRegion` > `凭据.region` > `config.authRegion` > `config.region`

**API Region**（API 请求）优先级：
`凭据.apiRegion` > `config.apiRegion` > `config.region`

注意凭据级 `region` 只影响 Auth Region，不参与 API Region 的回退。

### 代理配置

支持全局代理和凭据级代理，凭据级代理会覆盖该凭据产生的所有出站连接（API 请求、Token 刷新、额度查询）。

**代理优先级**：`凭据.proxyUrl` > `config.proxyUrl` > 无代理

| 凭据 `proxyUrl` 值 | 行为 |
|---|---|
| 具体 URL（如 `http://proxy:8080`、`socks5://proxy:1080`） | 使用凭据指定的代理 |
| `direct` | 显式不使用代理（即使全局配置了代理） |
| 未配置（留空） | 回退到全局代理配置 |

凭据级代理示例：

```json
[
   {
      "refreshToken": "凭据A：使用自己的代理",
      "authMethod": "social",
      "proxyUrl": "socks5://proxy-a.example.com:1080",
      "proxyUsername": "user_a",
      "proxyPassword": "pass_a"
   },
   {
      "refreshToken": "凭据B：显式不走代理（直连）",
      "authMethod": "social",
      "proxyUrl": "direct"
   },
   {
      "refreshToken": "凭据C：使用全局代理（或直连，取决于 config.json）",
      "authMethod": "social"
   }
]
```

启用 Admin 后还可以使用 [代理池](#代理池) 统一管理一批代理 IP，并在添加凭据时自动分配。

### 限流配置

限流规则以滑动窗口计数，达到上限的凭据在窗口内不会被选中（不会报错，只是让其他凭据接手；所有凭据都不可用时请求才失败）。

```json
"defaultRateLimits": [
   { "window": "1m", "maxRequests": 30 },
   { "window": "1h", "maxRequests": 500 }
]
```

- `window`：正整数加单位 `s` / `m` / `h` / `d`，最长 30 天，不支持小数
- `maxRequests`：必须大于 0
- 同一列表内不能有重复的窗口长度（`60s` 与 `1m` 视为重复）
- `config.defaultRateLimits` 是全局默认；凭据的 `rateLimits` 存在时**整体覆盖**全局默认，不做合并
- 规则非法时配置加载失败，服务不会启动

### 端点配置

支持三种 Kiro 上游端点，通过 `config.defaultEndpoint` 设置默认值，通过凭据的 `endpoint` 字段或 Admin API 按凭据覆盖。

| 端点 | API 地址 | 额度 / Profile 查询 | 说明 |
|------|----------|---------------------|------|
| `aws`（默认） | `https://q.<apiRegion>.amazonaws.com/generateAssistantResponse` | `q.<apiRegion>.amazonaws.com` | AWS 原版域名，请求格式与 `ide` 相同 |
| `ide` | `https://runtime.<apiRegion>.kiro.dev/generateAssistantResponse` | `management.<apiRegion>.kiro.dev` | 模拟 Kiro IDE（aws-sdk-js User-Agent，`x-amzn-kiro-agent-mode: vibe`） |
| `cli` | `https://runtime.<apiRegion>.kiro.dev/` | `management.<apiRegion>.kiro.dev` | 模拟 Kiro CLI（AWS JSON 1.0 协议、`x-amz-target` 头、aws-sdk-rust User-Agent，消息 `origin` 改为 `KIRO_CLI`） |

三种端点都会在请求体中注入凭据的 `profileArn`。

### 认证方式

客户端请求本服务时，支持两种认证方式：

1. **x-api-key Header**
   ```
   x-api-key: sk-your-api-key
   ```

2. **Authorization Bearer**
   ```
   Authorization: Bearer sk-your-api-key
   ```

### 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `RUST_LOG` | `info` | 日志级别，例如 `RUST_LOG=debug ./kiro-rs` |
| `KIRO_API_KEY` | - | 非空时在启动期插入一条 `priority: 0` 的 API Key 凭据到凭据列表最前面 |
| `KIRO_RS_IMAGE_RESIZE` | `1` | 设为 `0` / `false` / `no` / `off` 时关闭入站图片降采样 |
| `KIRO_RS_IMAGE_MAX_LONG_SIDE` | `1568` | 图片长边像素上限 |
| `KIRO_RS_IMAGE_MAX_BYTES` | `400000` | 图片 base64 字符数上限 |
| `KIRO_RS_IMAGE_JPEG_QUALITY` | `85` | 重编码 JPEG 的初始质量 |

编译期变量（由 `build.rs` 读取并注入二进制，Docker / GitHub Actions 构建时传入）：`KISSAPI_BUILD_TAG`（默认 `local`）、`KISSAPI_GIT_SHA`（默认取 `git rev-parse --short=7 HEAD`）。

### 运行时文件

服务会在 `credentials.json` 所在目录（Docker 场景为 `/app/config/`）写入以下文件：

| 文件 | 内容 |
|------|------|
| `kiro_stats.json` | 凭据成功 / 失败计数等统计（30 秒防抖落盘） |
| `kiro_balance_cache.json` | Admin 余额查询缓存（5 分钟） |
| `proxy_pool.json`（及 `.bak`） | 代理池数据，仅启用 Admin 时加载 |

## API 端点

### 标准端点 (/v1)

| 端点 | 方法 | 描述 |
|------|------|------|
| `/v1/models` | GET | 获取可用模型列表 |
| `/v1/messages` | POST | 创建消息（对话） |
| `/v1/messages/count_tokens` | POST | 估算 Token 数量（配置了 `countTokensApiUrl` 时优先调用外部 API，失败回退本地估算） |

请求体上限 50 MiB。

### Claude Code 兼容端点 (/cc/v1)

| 端点 | 方法 | 描述 |
|------|------|------|
| `/cc/v1/messages` | POST | 创建消息（缓冲模式，确保 `input_tokens` 准确） |
| `/cc/v1/messages/count_tokens` | POST | 估算 Token 数量（与 `/v1` 相同） |

> **`/cc/v1/messages` 与 `/v1/messages` 的区别**：
> - `/v1/messages`：实时流式返回，`message_start` 中的 `input_tokens` 是估算值；上游连续 90 秒没有产出任何文本 / thinking 内容时，会发送 `event: error`（`upstream_stall`）并断开，方便客户端重试
> - `/cc/v1/messages`：缓冲模式，等待上游流完成后，用从 `contextUsageEvent` 计算的准确 `input_tokens` 更正 `message_start`，然后一次性返回所有事件；没有静默看门狗
> - 两者等待期间都会每 25 秒发送 `ping` 事件保活

### Thinking 模式

支持 Claude 的 extended thinking 功能：

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 16000,
  "thinking": {
    "type": "enabled",
    "budget_tokens": 10000
  },
  "messages": [...]
}
```

行为说明：
- `thinking.type` 支持 `enabled` / `adaptive` / `disabled`；`budget_tokens` 缺省 20000，超过 24576 会被钳制到 24576
- 模型名带 `thinking` 后缀（如 `claude-opus-4-6-thinking`）会强制开启 thinking：Sonnet 4.6 / Opus 4.6 / 4.7 / 4.8 / 5 系列按 `adaptive`，其余按 `enabled`
- Opus 5 / Sonnet 5 收到 `enabled` 会自动改为 `adaptive`；这两个系列即使不传 `thinking` 也会解析响应中的 thinking 块
- effort 等级取自 `output_config.effort` 或 `effort.level`（`low` / `medium` / `high` / `xhigh` / `max`），未指定时由 `budget_tokens` 推导。只有 Sonnet 4.6、Opus 4.6 / 4.7 / 4.8、Sonnet 5、Opus 5 会下发原生 `output_config.effort`；不支持 `xhigh` 的模型会降为 `high`
- 非流式响应中的 `<thinking>` 标签提取由 `config.extractThinking` 控制

### 工具调用

完整支持 Anthropic 的 tool use 功能：

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "tools": [
    {
      "name": "get_weather",
      "description": "获取指定城市的天气",
      "input_schema": {
        "type": "object",
        "properties": {
          "city": {"type": "string"}
        },
        "required": ["city"]
      }
    }
  ],
  "messages": [...]
}
```

### 图片与文档

- 图片支持 `jpeg` / `png` / `webp` / `gif`。超过长边 1568 px 或 base64 400000 字符的图片会被降采样并重编码为 JPEG（GIF 直通不处理）；处理失败时原图直通。阈值可通过 [环境变量](#环境变量) 调整
- `document` 类型：`text/*` 直接解码为文本，PDF 抽取文字（最多 80000 字符），其他类型以占位文本代替

## 模型映射

请求中的 `model` 按小写子串匹配，按下表顺序命中第一条。上下文窗口用于 `/cc/v1` 精确 `input_tokens` 的换算。

| 请求模型名 | Kiro 模型 | 上下文窗口 |
|------------|-----------|------------|
| `gpt-5.6` / `gpt-5-6` / `gpt-5.6-sol` / `gpt-5-6-sol`（精确匹配） | `gpt-5.6-sol` | 272K |
| `gpt-5.6-terra` / `gpt-5-6-terra`（精确匹配） | `gpt-5.6-terra` | 272K |
| `gpt-5.6-luna` / `gpt-5-6-luna`（精确匹配） | `gpt-5.6-luna` | 272K |
| 其他含 `gpt` | 400 模型不支持 | - |
| 含 `sonnet` 且含 `sonnet-5` / `sonnet.5` | `claude-sonnet-5` | 1M |
| 含 `sonnet` 且含 `4-6` / `4.6` | `claude-sonnet-4.6` | 1M |
| 其他含 `sonnet` | `claude-sonnet-4.5` | 200K |
| 含 `opus` 且含 `opus-5` / `opus.5` | `claude-opus-5` | 1M |
| 含 `opus` 且含 `4-8` / `4.8` | `claude-opus-4.8` | 1M |
| 含 `opus` 且含 `4-7` / `4.7` | `claude-opus-4.7` | 1M |
| 含 `opus` 且含 `4-5` / `4.5` | `claude-opus-4.5` | 200K |
| 其他含 `opus` | `claude-opus-4.6` | 1M |
| 含 `haiku` | `claude-haiku-4.5` | 200K |
| 其他 | 400 模型不支持 | - |

`GET /v1/models` 公布的模型 ID（每个 Claude 模型都另有一个 `-thinking` 后缀变体）：`claude-opus-5`、`claude-sonnet-5`、`claude-opus-4-8`、`claude-opus-4-7`、`claude-opus-4-6`、`claude-sonnet-4-6`、`claude-opus-4-5-20251101`、`claude-sonnet-4-5-20250929`、`claude-haiku-4-5-20251001`，以及 `gpt-5.6-sol`、`gpt-5.6-terra`、`gpt-5.6-luna`。

## 多凭据与故障转移

多凭据故障转移发生在"取凭据"阶段，而不是"发请求"阶段：

1. 每个请求先由 Token 管理器选出一张可用凭据。`priority` 模式沿用当前凭据，不可用时取优先级最高者；`balanced` 模式每次取成功次数最少者。选择时自动跳过已禁用、正在风控冷却、触发限流规则、或不支持所请求模型（Free 订阅请求 Opus）的凭据。
2. 如果选中凭据的 Token 刷新失败，会累计该凭据的刷新失败次数（连续 3 次禁用）并换下一张继续，最多尝试 `凭据数 × 3` 次。
3. 选定后，请求只用这一张凭据发送**一次**。上游返回错误时直接把错误返给客户端，**不会**在同一请求内换凭据重发。
4. 上游错误会更新该凭据的状态，供后续请求避开：
   - 402 且额度耗尽（`MONTHLY_REQUEST_COUNT` / overage 用尽）：立即禁用
   - 401 / 403：累计失败，3 次后禁用
   - Kiro "suspicious activity" 429：进入冷却（默认 10 分钟，Admin 可调、可手动清除），不计失败
   - 408 / 其他 429 / 5xx：只记录，不禁用
5. 被自动禁用的凭据可在 Admin 中重置。若所有凭据都因连续失败被禁用，下一次请求会自动重置失败计数再试一轮。

只有内置 WebSearch（MCP）调用保留了请求级重试：最多 `min(凭据数 × 3, 9)` 次，会跨凭据切换。

## Admin（可选）

当 `config.json` 配置了非空 `adminApiKey` 时，会启用 Admin API 和 Admin UI。认证使用 `adminApiKey`，header 形式与业务 API 相同（`x-api-key` 或 `Authorization: Bearer`）。

### Admin API

挂载在 `/api/admin` 下：

| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/version` | 版本信息（版本号、代号、Git SHA、构建 tag、changelog） |
| GET | `/credentials` | 获取所有凭据状态 |
| POST | `/credentials` | 添加新凭据（可从代理池分配代理） |
| DELETE | `/credentials/{id}` | 删除凭据（同时释放代理池占用） |
| POST | `/credentials/{id}/disabled` | 设置凭据禁用状态 |
| POST | `/credentials/{id}/priority` | 设置凭据优先级 |
| PUT | `/credentials/{id}/display-name` | 设置凭据显示名称 |
| PUT | `/credentials/{id}/endpoint` | 切换凭据端点（`aws` / `ide` / `cli`，空值回退默认端点） |
| PUT | `/credentials/{id}/allow-overage` | 设置凭据超额模式 |
| PUT | `/credentials/{id}/rate-limits` | 设置凭据级限流规则 |
| POST | `/credentials/{id}/reset` | 重置失败计数并重新启用（同时清除冷却） |
| POST | `/credentials/{id}/clear-cooldown` | 手动清除运行时风控冷却 |
| POST | `/credentials/{id}/reset-stats` | 重置指定凭据的成功计数 |
| POST | `/credentials/reset-stats` | 重置所有凭据的成功计数 |
| POST | `/credentials/{id}/refresh` | 强制刷新 Token（API Key 凭据返回 400） |
| GET | `/credentials/{id}/balance` | 获取凭据余额（5 分钟缓存） |
| GET / PUT | `/config/load-balancing` | 查看 / 设置负载均衡模式 |
| GET / PUT | `/config/settings` | 查看 / 设置运行时设置（`suspiciousCooldownMinutes`、`extractThinking`、`nativeLikeTwoPhaseFlow`），修改会持久化到 `config.json` |
| GET | `/proxy-pool` | 代理池列表、统计与设置 |
| POST | `/proxy-pool` | 添加单个代理 |
| POST | `/proxy-pool/batch` | 批量导入代理 |
| GET / PUT | `/proxy-pool/settings` | 查看 / 设置代理池设置（`autoAssignEnabled`、`probeUrl`） |
| PUT | `/proxy-pool/{id}` | 更新代理 |
| DELETE | `/proxy-pool/{id}` | 删除代理（仍被凭据占用时拒绝） |
| POST | `/proxy-pool/{id}/disabled` | 启用 / 禁用代理 |
| POST | `/proxy-pool/{id}/test` | 探测代理连通性 |

### Admin UI

`GET /admin` 访问管理页面（需要在编译前构建 `admin-ui/dist`）。提供凭据卡片（状态、邮箱、订阅、余额、冷却、最近请求状态）、单条 / 批量导入（JSON 数组或 KAM 导出格式）、批量验活、限流与端点设置、代理池管理、运行时设置和版本徽标。登录用的 Admin Key 保存在浏览器 `localStorage` 中。

### 代理池

代理池用于集中管理一批出口代理，并在**添加凭据时**分配给凭据：分配后代理的地址和账号直接写入凭据的 `proxyUrl` / `proxyUsername` / `proxyPassword`，请求路径上仍按 [代理配置](#代理配置) 的规则生效，代理池本身不参与请求。

- 数据持久化在 `proxy_pool.json`（写入前备份 `.bak`，解析失败自动回退）
- 添加凭据时若没有手填 `proxyUrl`：指定 `proxyId` 则手动分配（默认只允许空闲代理，`proxyAllowReuse` 可复用），否则在 `autoAssignEnabled` 开启时自动分配负载最低的代理（无空闲时复用在用代理）。分配前会探测，探测失败默认不分配
- 探测通过代理访问 `probeUrl`（默认 `https://api.ipify.org?format=json`，必须是 https 公网地址），成功结果缓存 5 分钟、失败缓存 60 秒；没有后台定时巡检
- 支持 `http` / `https` / `socks5` / `socks5h` 代理
- 批量导入每行一条，支持 `url [username] [password] [label]`（空格分隔）或 `host:port[:username:password]`（无协议前缀时默认 `socks5://`）

## 注意事项

1. **凭证安全**: 请妥善保管 `credentials.json` 和 `proxy_pool.json`（含代理明文密码），不要提交到版本控制
2. **Token 刷新**: 服务会自动刷新过期的 Token，无需手动干预
3. **WebSearch 工具**: 当 `tools` 列表仅包含一个 `web_search` 工具时，会走内置 WebSearch 转换逻辑
4. **Opus 与 Free 订阅**: `subscriptionTitle` 含 `FREE` 的凭据不会被分配 Opus 请求；订阅信息在查询余额时更新

## 项目结构

```
kiro-rs/
├── src/
│   ├── main.rs                 # 程序入口：加载配置与凭据、注册端点、组装路由
│   ├── http_client.rs          # HTTP 客户端构建（TLS 后端、代理）
│   ├── token.rs                # count_tokens：外部 API 优先，失败回退本地估算
│   ├── image_resize.rs         # 入站图片降采样
│   ├── proxy_pool.rs           # IP 代理池（Admin）
│   ├── version.rs              # 版本信息（由 build.rs 注入）
│   ├── model/                  # 配置和参数模型
│   │   ├── config.rs           # 应用配置
│   │   ├── arg.rs              # 命令行参数
│   │   └── rate_limit.rs       # 限流规则解析与校验
│   ├── anthropic/              # Anthropic API 兼容层
│   │   ├── router.rs           # 路由配置
│   │   ├── handlers.rs         # 请求处理器
│   │   ├── middleware.rs       # 认证中间件
│   │   ├── planner.rs          # 请求执行计划（单阶段 / 双阶段）
│   │   ├── executor.rs         # 取凭据、调用上游、组装响应
│   │   ├── types.rs            # 类型定义
│   │   ├── converter.rs        # 协议转换器（Anthropic → Kiro）
│   │   ├── stream.rs           # 流式响应处理（Kiro 事件 → Anthropic SSE）
│   │   └── websearch.rs        # WebSearch 工具处理
│   ├── kiro/                   # Kiro API 客户端
│   │   ├── provider.rs         # 上游请求发送
│   │   ├── token_manager.rs    # 多凭据管理、Token 刷新、状态回写
│   │   ├── machine_id.rs       # 设备指纹生成
│   │   ├── endpoint/           # aws / ide / cli 三种上游端点
│   │   ├── model/              # 数据模型
│   │   │   ├── credentials.rs  # OAuth 凭证
│   │   │   ├── events/         # 响应事件类型
│   │   │   ├── requests/       # 请求类型
│   │   │   ├── common/         # 共享类型
│   │   │   ├── token_refresh.rs # Token 刷新模型
│   │   │   └── usage_limits.rs # 使用额度模型
│   │   └── parser/             # AWS Event Stream 解析器
│   │       ├── decoder.rs      # 流式解码器
│   │       ├── frame.rs        # 帧解析
│   │       ├── header.rs       # 头部解析
│   │       ├── error.rs        # 错误类型
│   │       └── crc.rs          # CRC 校验
│   ├── admin/                  # Admin API 模块
│   │   ├── router.rs           # 路由配置
│   │   ├── handlers.rs         # 请求处理器
│   │   ├── service.rs          # 业务逻辑服务
│   │   ├── types.rs            # 类型定义
│   │   ├── middleware.rs       # 认证中间件
│   │   └── error.rs            # 错误处理
│   ├── admin_ui/               # Admin UI 静态文件嵌入
│   │   └── router.rs           # 静态文件路由
│   └── common/                 # 公共模块
│       └── auth.rs             # 认证工具函数
├── admin-ui/                   # Admin UI 前端工程（构建产物会嵌入二进制）
├── tools/event-viewer.html     # 浏览器端 AWS Event Stream 调试工具
├── docs/KISSAPI_CHANGELOG.md   # 二次开发分支变更记录
├── build.rs                    # 编译期注入版本信息
├── VERSION.json                # 二次开发分支版本源
├── Cargo.toml                  # 项目配置
├── config.example.json         # 配置示例
├── credentials.example.*.json  # 凭据示例（social / idc / apikey / multiple）
├── docker-compose.yml          # Docker Compose 配置
└── Dockerfile                  # Docker 构建文件
```

## 技术栈

- **Web 框架**: [Axum](https://github.com/tokio-rs/axum) 0.8
- **异步运行时**: [Tokio](https://tokio.rs/)
- **HTTP 客户端**: [Reqwest](https://github.com/seanmonstar/reqwest)
- **序列化**: [Serde](https://serde.rs/)
- **日志**: [tracing](https://github.com/tokio-rs/tracing)
- **命令行**: [Clap](https://github.com/clap-rs/clap)
- **静态资源嵌入**: [rust-embed](https://github.com/pyrossh/rust-embed)
- **Admin UI**: React 18 + Vite + Tailwind CSS + Radix UI

## License

MIT

## 致谢

本项目的实现离不开前辈的努力:  
 - [kiro2api](https://github.com/caidaoli/kiro2api)
 - [proxycast](https://github.com/aiclientproxy/proxycast)

本项目部分逻辑参考了以上的项目, 再次由衷的感谢!
