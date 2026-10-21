"""Presentazione conservativa dei testi scriptati, senza eseguire script HOI4.

Le stringhe restano testo: il chiamante deve usare textContent, mai innerHTML.
I riferimenti dinamici, le condizioni e i parametri del motore non sono valutati.
"""
from __future__ import annotations

from decimal import Decimal, InvalidOperation
import re

from .clausewitz import Node

MAX_LOCALISATION_DEPTH = 12
MAX_LOCALISATION_EXPANSIONS = 512
MAX_LOCALISATION_LENGTH = 16_384
_TRUNCATED = "\n[Localizzazione troncata: limite di lunghezza]"
_REFERENCE = re.compile(r"(?<!\\)\$([A-Za-z0-9_.:@-]+)(?:\|([^$\n]*))?\$")
_ENGINE_PARAMETERS = frozenset({"RIGHT", "LEFT", "VALUE", "COUNT", "DAYS", "NAME", "CHARACTER"})
_COLORS = frozenset("YRGBWgLbOHCMr")


def localise(text: str, labels: dict) -> str:
    """Espande solo chiavi statiche note, mantenendo letterali gli altri token.

    Cicli, profondità o budget esauriti lasciano il riferimento originale.
    Il limite finale di caratteri produce un avviso visibile. Nessun escape HTML
    viene decodificato e nessun contenuto delle stringhe diventa codice.
    """
    remaining = MAX_LOCALISATION_EXPANSIONS

    def bounded(value: str) -> str:
        if len(value) > MAX_LOCALISATION_LENGTH:
            return value[:MAX_LOCALISATION_LENGTH - len(_TRUNCATED)] + _TRUNCATED
        return value

    def expand(value: str, stack: frozenset[str], depth: int) -> str:
        nonlocal remaining
        value = bounded(value)
        if depth >= MAX_LOCALISATION_DEPTH:
            return value

        def replace(match: re.Match) -> str:
            nonlocal remaining
            key, fmt = match.group(1), match.group(2)
            if (remaining <= 0 or key in stack or key in _ENGINE_PARAMETERS
                    or key not in labels or not isinstance(labels[key], str)):
                return match.group(0)
            # Only a single colour directive is static. Numeric engine formats
            # (e.g. RIGHT|+=%1) require values unavailable to this application.
            if fmt is not None and fmt not in _COLORS:
                return match.group(0)
            remaining -= 1
            result = expand(labels[key], stack | {key}, depth + 1)
            return f"§{fmt}{result}§!" if fmt else result

        return bounded(_REFERENCE.sub(replace, value))

    return expand(str(text), frozenset(), 0)


_NUMERIC_EFFECTS = {
    "add_political_power": ("Potere politico", False),
    "add_stability": ("Stabilità", True),
    "add_war_support": ("Sostegno bellico", True),
    "add_manpower": ("Risorse umane", False),
    "army_experience": ("Esperienza dell'esercito", False),
    "navy_experience": ("Esperienza della marina", False),
    "air_experience": ("Esperienza dell'aviazione", False),
    "add_research_slot": ("Slot di ricerca", False),
    "add_command_power": ("Potere di comando", False),
}
_CONDITIONAL = frozenset({"if", "else", "else_if", "random", "random_list", "while",
                          "every_country", "every_state", "random_country", "random_state"})
_SPACER_KEYS = frozenset({"generic_skip_one_line_tt", "generic_skip_two_lines_tt"})


def _numeric_effect(key: str, value: str) -> str | None:
    if key not in _NUMERIC_EFFECTS:
        return None
    try:
        number = Decimal(value)
    except InvalidOperation:
        return None
    if not number.is_finite():
        return None
    if number and (number.adjusted() > 15 or number.adjusted() < -10):
        return None
    title, percent = _NUMERIC_EFFECTS[key]
    number *= 100 if percent else 1
    # Avoid expanding malicious or malformed exponent strings into huge output.
    if abs(number) > Decimal("1e15") or (number and abs(number) < Decimal("1e-8")):
        return None
    rendered = format(number.normalize(), "f")
    return f"{title}: {'+' if number > 0 else ''}{rendered}{'%' if percent else ''}"


def focus_tooltips(node: Node, labels: dict) -> list[dict]:
    """Testi diretti delle sezioni; non decide se trigger o effetti si applicano.

    Un custom_trigger_tooltip diretto è una presentazione dichiarata dal gioco,
    non una valutazione del suo contenuto. Gli altri blocchi annidati non sono
    appiattiti, soprattutto IF/ELSE e hidden_effect.
    """
    sections = []
    for section, heading in (("available", "Requisiti"), ("bypass", "Bypass"),
                             ("completion_reward", "Effetti")):
        lines: list[str] = []
        has_unrepresented = False
        has_conditional = False

        def add_tooltip(key: str) -> bool:
            if key in _SPACER_KEYS:
                return True
            if key not in labels or not isinstance(labels[key], str):
                return False
            text = localise(labels[key], labels).strip()
            if text:
                lines.append(text)
            return True

        for block in node.blocks(section):
            for key, value in block.entries:
                lower_key = key.lower()
                if isinstance(value, str):
                    if key in ("custom_effect_tooltip", "tooltip"):
                        has_unrepresented |= not add_tooltip(value)
                    elif section == "completion_reward" and (effect := _numeric_effect(key, value)):
                        lines.append(effect)
                    elif section == "completion_reward" and key in ("show_ideas_tooltip", "add_ideas", "remove_ideas") and value in labels:
                        title = {"show_ideas_tooltip": "Idea o consigliere", "add_ideas": "Aggiunge idea", "remove_ideas": "Rimuove idea"}[key]
                        lines.append(f"{title}: {localise(labels[value], labels)}")
                    else:
                        has_unrepresented = True
                elif key == "custom_trigger_tooltip" and section != "completion_reward":
                    # The trigger's own label is safe to display; its truth is unknown.
                    has_unrepresented |= not add_tooltip(value.scalar("tooltip"))
                else:
                    has_unrepresented = True
                    has_conditional |= lower_key in _CONDITIONAL or lower_key in ("hidden_effect", "hidden_trigger")

        if section == "available" and any(node.blocks(key) for key in ("allow_branch", "visible")):
            has_unrepresented = True
        if has_conditional:
            lines.append("Altri effetti condizionali da verificare nel gioco" if section == "completion_reward"
                         else "Condizioni aggiuntive da verificare nel gioco")
        elif has_unrepresented:
            lines.append("Altri effetti da verificare nel gioco" if section == "completion_reward"
                         else "Condizioni aggiuntive da verificare nel gioco")
        if lines:
            sections.append({"heading": heading, "text": "\n".join(lines)})
    return sections
