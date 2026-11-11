"""python -m percent_grading prompt CASE | aggregate CASE JUDGMENTS"""

import argparse
import json
import sys

from .core import ValidationError, aggregate, load_json, make_prompt


def main(argv=None):
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description="Legacy teaching prototype: provisional profiles only. Official grades require the current Rust examiner-review workflow.")
    sub = parser.add_subparsers(dest="command", required=True)
    prompt = sub.add_parser("prompt", help="Print an evaluation request for an AI judge")
    prompt.add_argument("case")
    score = sub.add_parser("aggregate", help="Validate judgments and aggregate deterministically")
    score.add_argument("case")
    score.add_argument("judgments")
    args = parser.parse_args(argv)
    try:
        case = load_json(args.case)
        if args.command == "prompt":
            print(make_prompt(case))
        else:
            result = aggregate(case, load_json(args.judgments))
            print(json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False))
        return 0
    except (ValidationError, OSError, ValueError) as exc:
        print(f"Invalid input: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
