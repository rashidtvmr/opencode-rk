#!/usr/bin/env python3
"""Minimal VT100/xterm screen model used by the OC2 parity renderers.

Implements the subset emitted by the pinned OpenTUI native renderer:
cursor addressing, SGR truecolor/256/basic colors, line/char erase, insert and
delete line/char, autowrap, alternate-screen toggles, and OSC/DCS/APC skipping.
Wide cells keep an explicit continuation column (PAR-05).
"""

from __future__ import annotations

import codecs
import unicodedata

ATTR_BITS = (
    "bold",
    "dim",
    "italic",
    "underline",
    "blink",
    "reverse",
    "hidden",
    "strike",
)

BASIC = (
    "#000000",
    "#800000",
    "#008000",
    "#808000",
    "#000080",
    "#800080",
    "#008080",
    "#c0c0c0",
)
BRIGHT = (
    "#808080",
    "#ff0000",
    "#00ff00",
    "#ffff00",
    "#0000ff",
    "#ff00ff",
    "#00ffff",
    "#ffffff",
)


def char_width(ch: str) -> int:
    if not ch:
        return 0
    if unicodedata.combining(ch):
        return 0
    if unicodedata.category(ch) in ("Mn", "Me", "Cf"):
        return 0
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def palette_256(index: int) -> str:
    if index < 16:
        return (BASIC + BRIGHT)[index]
    if index < 232:
        value = index - 16
        red = value // 36
        green = (value % 36) // 6
        blue = value % 6

        def step(level: int) -> int:
            return 0 if level == 0 else 55 + level * 40

        return f"#{step(red):02x}{step(green):02x}{step(blue):02x}"
    level = 8 + (index - 232) * 10
    return f"#{level:02x}{level:02x}{level:02x}"


class Screen:
    def __init__(self, cols: int, rows: int, default_fg="#ffffff", default_bg="#000000"):
        self.cols = cols
        self.rows = rows
        self.default_fg = default_fg
        self.default_bg = default_bg
        self.grid = [[self._blank() for _ in range(cols)] for _ in range(rows)]
        self.row = 0
        self.col = 0
        self.saved = (0, 0)
        self.visible = False
        self.autowrap = True
        self.fg = default_fg
        self.bg = default_bg
        self.attrs = set()
        self._decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self._state = "ground"
        self._buffer = ""
        self._string_esc = False
        self.scrolled = 0

    def _blank(self):
        return {
            "ch": " ",
            "fg": self.default_fg,
            "bg": self.default_bg,
            "attrs": frozenset(),
            "cont": False,
        }

    def _style(self):
        return {
            "fg": self.fg,
            "bg": self.bg,
            "attrs": frozenset(self.attrs),
            "cont": False,
        }

    def _is_default(self, cell):
        return (
            cell["fg"] == self.default_fg
            and cell["bg"] == self.default_bg
            and not cell["attrs"]
        )

    def feed(self, data: bytes) -> None:
        for ch in self._decoder.decode(data):
            self._consume(ch)

    def _consume(self, ch: str) -> None:
        state = self._state
        if state == "ground":
            if ch == "\x1b":
                self._state = "esc"
            elif ch == "\r":
                self.col = 0
            elif ch == "\n":
                self._linefeed()
            elif ch == "\b":
                self.col = max(0, self.col - 1)
            elif ch == "\t":
                self.col = min(self.cols - 1, ((self.col // 8) + 1) * 8)
            elif ch == "\x07" or ord(ch) < 0x20:
                return
            else:
                self._write(ch)
            return
        if state == "esc":
            self._state = "ground"
            if ch == "[":
                self._state = "csi"
                self._buffer = ""
            elif ch == "]":
                self._state = "osc"
                self._buffer = ""
            elif ch in ("P", "^", "_", "X"):
                self._state = "string"
                self._string_esc = False
            elif ch == "7":
                self.saved = (self.row, self.col)
            elif ch == "8":
                self.row, self.col = self.saved
                self._clamp_cursor()
            elif ch == "M":
                self._reverse_index()
            elif ch == "D":
                self._linefeed()
            elif ch == "E":
                self._linefeed()
                self.col = 0
            elif ch == "c":
                self._reset()
            return
        if state == "csi":
            if "\x40" <= ch <= "\x7e":
                self._csi(self._buffer, ch)
                self._buffer = ""
                self._state = "ground"
            else:
                self._buffer += ch
            return
        if state == "osc":
            if ch == "\x07":
                self._state = "ground"
            elif ch == "\x1b":
                self._state = "string"
                self._string_esc = True
            return
        if state == "string":
            if self._string_esc:
                if ch == "\\":
                    self._state = "ground"
                    self._string_esc = False
                else:
                    self._string_esc = False
                return
            if ch == "\x1b":
                self._string_esc = True
            return

    def _reset(self) -> None:
        self.grid = [[self._blank() for _ in range(self.cols)] for _ in range(self.rows)]
        self.row = 0
        self.col = 0
        self.saved = (0, 0)
        self.visible = True
        self.autowrap = True
        self.fg = self.default_fg
        self.bg = self.default_bg
        self.attrs.clear()

    def _clamp_cursor(self) -> None:
        self.row = max(0, min(self.rows - 1, self.row))
        self.col = max(0, min(self.cols, self.col))

    def _linefeed(self) -> None:
        if self.row + 1 < self.rows:
            self.row += 1
        else:
            self.grid.pop(0)
            self.grid.append([self._blank() for _ in range(self.cols)])
            self.scrolled += 1

    def _reverse_index(self) -> None:
        if self.row > 0:
            self.row -= 1
        else:
            self.grid.pop()
            self.grid.insert(0, [self._blank() for _ in range(self.cols)])

    def _write(self, ch: str) -> None:
        width = char_width(ch)
        if width == 0:
            # Combining mark: attach to the previous base cell.
            if self.col > 0:
                self.grid[self.row][min(self.cols - 1, self.col - 1)]["ch"] += ch
            return
        if self.col >= self.cols:
            if self.autowrap:
                self.col = 0
                self._linefeed()
            else:
                self.col = self.cols - 1
        style = self._style()
        cell = self.grid[self.row][self.col]
        cell.update(style)
        cell["ch"] = ch
        if width == 2 and self.col + 1 < self.cols:
            cont = self.grid[self.row][self.col + 1]
            cont.update(style)
            cont["ch"] = ""
            cont["cont"] = True
        self.col += width
        if not self.autowrap and self.col >= self.cols:
            self.col = self.cols - 1

    def _csi(self, raw: str, final: str) -> None:
        private = raw[:1] in ("?", ">", "<", "=")
        body = raw[1:] if private else raw
        parts = body.split(";") if body else []
        nums = []
        for part in parts:
            if part.isdigit():
                nums.append(int(part))
            else:
                nums.append(0)

        def arg(index: int, default: int = 0) -> int:
            return nums[index] if index < len(nums) else default

        if final in ("H", "f"):
            self.row = max(0, arg(0, 1) - 1)
            self.col = max(0, arg(1, 1) - 1)
            self._clamp_cursor()
        elif final == "A":
            self.row -= max(1, arg(0, 1))
            self._clamp_cursor()
        elif final == "B":
            self.row += max(1, arg(0, 1))
            self._clamp_cursor()
        elif final == "C":
            self.col += max(1, arg(0, 1))
            self._clamp_cursor()
        elif final == "D":
            self.col -= max(1, arg(0, 1))
            self._clamp_cursor()
        elif final == "E":
            self.row += max(1, arg(0, 1))
            self.col = 0
            self._clamp_cursor()
        elif final == "F":
            self.row -= max(1, arg(0, 1))
            self.col = 0
            self._clamp_cursor()
        elif final == "G":
            self.col = max(0, arg(0, 1) - 1)
            self._clamp_cursor()
        elif final == "d":
            self.row = max(0, arg(0, 1) - 1)
            self._clamp_cursor()
        elif final == "J":
            self._erase_display(arg(0, 0))
        elif final == "K":
            self._erase_line(arg(0, 0))
        elif final == "m":
            self._sgr(nums if nums else [0])
        elif final in ("h", "l"):
            enabled = final == "h"
            for mode in nums:
                if mode == 25:
                    self.visible = enabled
                elif mode == 7:
                    self.autowrap = enabled
        elif final == "s":
            self.saved = (self.row, self.col)
        elif final == "u":
            self.row, self.col = self.saved
            self._clamp_cursor()
        elif final == "L":
            for _ in range(max(1, arg(0, 1))):
                self.grid.insert(self.row, [self._blank() for _ in range(self.cols)])
                self.grid.pop()
        elif final == "M":
            for _ in range(max(1, arg(0, 1))):
                self.grid.pop(self.row)
                self.grid.append([self._blank() for _ in range(self.cols)])
        elif final == "P":
            count = max(1, arg(0, 1))
            row = self.grid[self.row]
            del row[self.col : self.col + count]
            row.extend([self._blank() for _ in range(count)])
        elif final == "@":
            count = max(1, arg(0, 1))
            row = self.grid[self.row]
            for _ in range(count):
                row.insert(self.col, self._blank())
            del row[self.cols :]
        elif final == "X":
            count = max(1, arg(0, 1))
            for col in range(self.col, min(self.cols, self.col + count)):
                self.grid[self.row][col] = self._blank()

    def _erase_display(self, mode: int) -> None:
        if mode in (2, 3):
            for row in range(self.rows):
                for col in range(self.cols):
                    self.grid[row][col] = self._blank()
            return
        if mode == 0:
            for col in range(self.col, self.cols):
                self.grid[self.row][col] = self._blank()
            for row in range(self.row + 1, self.rows):
                for col in range(self.cols):
                    self.grid[row][col] = self._blank()
        elif mode == 1:
            for row in range(0, self.row):
                for col in range(self.cols):
                    self.grid[row][col] = self._blank()
            for col in range(0, min(self.col + 1, self.cols)):
                self.grid[self.row][col] = self._blank()

    def _erase_line(self, mode: int) -> None:
        if mode == 0:
            for col in range(self.col, self.cols):
                self.grid[self.row][col] = self._blank()
        elif mode == 1:
            for col in range(0, min(self.col + 1, self.cols)):
                self.grid[self.row][col] = self._blank()
        else:
            for col in range(self.cols):
                self.grid[self.row][col] = self._blank()

    def _sgr(self, nums) -> None:
        index = 0
        while index < len(nums):
            code = nums[index]
            if code == 0:
                self.fg = self.default_fg
                self.bg = self.default_bg
                self.attrs.clear()
            elif code == 1:
                self.attrs.add("bold")
            elif code == 2:
                self.attrs.add("dim")
            elif code == 3:
                self.attrs.add("italic")
            elif code == 4:
                self.attrs.add("underline")
            elif code == 5:
                self.attrs.add("blink")
            elif code == 7:
                self.attrs.add("reverse")
            elif code == 8:
                self.attrs.add("hidden")
            elif code == 9:
                self.attrs.add("strike")
            elif code == 22:
                self.attrs.discard("bold")
                self.attrs.discard("dim")
            elif code == 23:
                self.attrs.discard("italic")
            elif code == 24:
                self.attrs.discard("underline")
            elif code == 25:
                self.attrs.discard("blink")
            elif code == 27:
                self.attrs.discard("reverse")
            elif code == 28:
                self.attrs.discard("hidden")
            elif code == 29:
                self.attrs.discard("strike")
            elif 30 <= code <= 37:
                self.fg = BASIC[code - 30]
            elif code == 39:
                self.fg = self.default_fg
            elif 40 <= code <= 47:
                self.bg = BASIC[code - 40]
            elif code == 49:
                self.bg = self.default_bg
            elif 90 <= code <= 97:
                self.fg = BRIGHT[code - 90]
            elif 100 <= code <= 107:
                self.bg = BRIGHT[code - 100]
            elif code in (38, 48):
                target = "fg" if code == 38 else "bg"
                if index + 1 < len(nums) and nums[index + 1] == 2 and index + 4 < len(nums):
                    red, green, blue = nums[index + 2 : index + 5]
                    setattr(self, target, f"#{red:02x}{green:02x}{blue:02x}")
                    index += 4
                elif index + 1 < len(nums) and nums[index + 1] == 5 and index + 2 < len(nums):
                    setattr(self, target, palette_256(nums[index + 2]))
                    index += 2
            index += 1

    def snapshot(self):
        rows = []
        for row_index in range(self.rows):
            cells = self.grid[row_index]
            keep = self.cols
            while keep > 0:
                cell = cells[keep - 1]
                if cell["ch"] == " " and not cell["cont"] and self._is_default(cell):
                    keep -= 1
                else:
                    break
            text_parts = []
            spans = []
            run = None
            for col in range(keep):
                cell = cells[col]
                if not cell["cont"]:
                    text_parts.append(cell["ch"])
                style = (
                    cell["fg"],
                    cell["bg"],
                    tuple(sorted(cell["attrs"])),
                )
                if style == (self.default_fg, self.default_bg, ()):
                    if run:
                        spans.append(run)
                        run = None
                    continue
                if run and run["_style"] == style and run["c"] + run["n"] == col:
                    run["n"] += 1
                else:
                    if run:
                        spans.append(run)
                    run = {
                        "c": col,
                        "n": 1,
                        "fg": cell["fg"],
                        "bg": cell["bg"],
                        "attrs": list(sorted(cell["attrs"])),
                        "_style": style,
                    }
            if run:
                spans.append(run)
            clean_spans = []
            for span in spans:
                span = dict(span)
                span.pop("_style", None)
                clean_spans.append(span)
            rows.append({"text": "".join(text_parts), "w": self.cols, "spans": clean_spans})
        row = max(0, min(self.rows - 1, self.row))
        col = max(0, min(self.cols, self.col))
        return {
            "viewport": {"cols": self.cols, "rows": self.rows},
            "default_style": {"fg": self.default_fg, "bg": self.default_bg},
            "cursor": {"row": row, "col": col, "visible": self.visible},
            "focus": None,
            "rows": rows,
        }
