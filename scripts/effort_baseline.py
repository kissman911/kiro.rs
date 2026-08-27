#!/usr/bin/env python3
"""effort 阶梯行为基线采样器。

背景：claude-opus-4.6 的 thinking 输出方差极大——同一配置两次采样可以
从 4677 思考字符掉到 0。单次采样不足以支撑任何结论，也无法作为重构的
行为基线。本脚本对每个配置重复 N 次，取中位数并记录全部原始样本。

用法（在 DMIT2 上跑，key 从环境变量取）：
    export KRK=$(python3 -c 'import json;print(json.load(open("/opt/kiro-rs-opus47/config/config.json"))["apiKey"])')
    python3 effort_baseline.py --port 8992 --reps 5 --out baseline.json

只读：脚本只发推理请求，不改任何配置或容器状态。
"""

import argparse
import json
import os
import statistics
import sys
import time
import urllib.error
import urllib.request

# 固定题目：需要真实推理但答案短，避免 max_tokens 截断干扰思考量统计。
PROMPT = (
    "严谨推理：一个池塘的睡莲每天覆盖面积翻倍，第 48 天恰好铺满整个池塘。"
    "第几天铺满一半？并解释为什么直觉容易答错。"
)


def build_configs():
    """返回 (label, model, thinking, output_config) 四元组列表。

    覆盖三类问题：
    - 4.6 的 effort 阶梯是否真的单调（low/high/xhigh/max）
    - 4.6-thinking 别名实际落在哪一档
    - opus-5 作为「默认开 thinking」的参照基准
    """
    m46 = "claude-opus-4-6"
    adaptive = {"type": "adaptive"}
    return [
        ("4.6 adaptive+low", m46, adaptive, {"effort": "low"}),
        ("4.6 adaptive+high", m46, adaptive, {"effort": "high"}),
        ("4.6 adaptive+xhigh", m46, adaptive, {"effort": "xhigh"}),
        ("4.6 adaptive+max", m46, adaptive, {"effort": "max"}),
        ("4.6-thinking alias", "claude-opus-4-6-thinking", None, None),
        ("opus-5 bare", "claude-opus-5", None, None),
    ]


def sample_once(url, key, model, thinking, output_config, stream, timeout):
    """发一次请求，返回 (thinking_chars, text_chars, elapsed_s, error)。

    流式统计 thinking_delta；非流式统计 thinking content_block。
    两者都只看最终暴露给客户端的 thinking 量，这才是用户实际感知到的。
    """
    body = {
        "model": model,
        "max_tokens": 4000,
        "stream": stream,
        "messages": [{"role": "user", "content": PROMPT}],
    }
    if thinking:
        body["thinking"] = thinking
    if output_config:
        body["output_config"] = output_config

    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"x-api-key": key, "content-type": "application/json"},
    )

    t0 = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            if stream:
                th = txt = 0
                for raw in resp:
                    line = raw.decode("utf-8", "replace").strip()
                    if not line.startswith("data:"):
                        continue
                    try:
                        evt = json.loads(line[5:].strip())
                    except ValueError:
                        continue
                    if evt.get("type") != "content_block_delta":
                        continue
                    delta = evt.get("delta", {})
                    if delta.get("type") == "thinking_delta":
                        th += len(delta.get("thinking", ""))
                    elif delta.get("type") == "text_delta":
                        txt += len(delta.get("text", ""))
                return th, txt, time.time() - t0, None

            payload = json.loads(resp.read())
            blocks = payload.get("content", [])
            th = sum(len(b.get("thinking", "")) for b in blocks if b.get("type") == "thinking")
            txt = sum(len(b.get("text", "")) for b in blocks if b.get("type") == "text")
            return th, txt, time.time() - t0, None
    except urllib.error.HTTPError as e:
        detail = e.read()[:200].decode("utf-8", "replace")
        return 0, 0, time.time() - t0, f"HTTP{e.code}: {detail}"
    except Exception as e:  # 网络/超时/解析
        return 0, 0, time.time() - t0, f"{type(e).__name__}: {e}"


def summarize(samples):
    """中位数抗离群，同时保留 min/max 暴露方差。"""
    vals = [s["thinking_chars"] for s in samples if s["error"] is None]
    if not vals:
        return None
    return {
        "n": len(vals),
        "median": statistics.median(vals),
        "mean": round(statistics.mean(vals), 1),
        "min": min(vals),
        "max": max(vals),
        "zero_rate": round(sum(1 for v in vals if v == 0) / len(vals), 2),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8992)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--timeout", type=int, default=300)
    ap.add_argument("--modes", default="stream", help="stream / nonstream / both")
    ap.add_argument("--out", default="baseline.json")
    args = ap.parse_args()

    key = os.environ.get("KRK")
    if not key:
        print("需要环境变量 KRK（kiro-rs apiKey）", file=sys.stderr)
        return 2

    url = f"http://{args.host}:{args.port}/v1/messages"
    modes = ["stream", "nonstream"] if args.modes == "both" else [args.modes]

    result = {
        "meta": {
            "url": url,
            "reps": args.reps,
            "prompt": PROMPT,
            "started_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        },
        "configs": [],
    }

    for mode in modes:
        stream = mode == "stream"
        for label, model, thinking, output_config in build_configs():
            samples = []
            for i in range(args.reps):
                th, txt, elapsed, err = sample_once(
                    url, key, model, thinking, output_config, stream, args.timeout
                )
                samples.append(
                    {
                        "rep": i + 1,
                        "thinking_chars": th,
                        "text_chars": txt,
                        "elapsed_s": round(elapsed, 1),
                        "error": err,
                    }
                )
                flag = f" ERR {err}" if err else ""
                print(
                    f"[{mode}] {label:22s} rep{i+1}/{args.reps} "
                    f"think={th:6d} text={txt:5d} t={elapsed:5.1f}s{flag}",
                    flush=True,
                )

            entry = {
                "mode": mode,
                "label": label,
                "model": model,
                "thinking": thinking,
                "output_config": output_config,
                "samples": samples,
                "summary": summarize(samples),
            }
            result["configs"].append(entry)
            s = entry["summary"]
            if s:
                print(
                    f"  -> {label}: median={s['median']} mean={s['mean']} "
                    f"range=[{s['min']},{s['max']}] zero_rate={s['zero_rate']}",
                    flush=True,
                )

    with open(args.out, "w") as f:
        json.dump(result, f, ensure_ascii=False, indent=2)

    print(f"\n=== 汇总（中位数 thinking 字符）===")
    for e in result["configs"]:
        s = e["summary"]
        if s:
            print(
                f"{e['mode']:9s} {e['label']:22s} median={s['median']:7.1f} "
                f"range=[{s['min']},{s['max']}] zero={s['zero_rate']}"
            )
        else:
            print(f"{e['mode']:9s} {e['label']:22s} 全部失败")
    print(f"\n基线已写入 {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
