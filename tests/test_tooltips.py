import unittest

from hoi4companion.clausewitz import parse
from hoi4companion.tooltips import (
    MAX_LOCALISATION_LENGTH, focus_tooltips, localise,
)


class LocalisationTest(unittest.TestCase):
    def test_nested_static_keys_preserve_game_markup(self):
        labels = {"title": "§Y$factory$§!\n£resources_strip|5 [ROOT.GetName]", "factory": "Fabbrica"}
        self.assertEqual(localise("$title$", labels), "§YFabbrica§!\n£resources_strip|5 [ROOT.GetName]")
        self.assertEqual(localise("$factory|G$", labels), "§GFabbrica§!")

    def test_engine_parameters_and_dynamic_values_stay_literal(self):
        text = "$RIGHT|+=%1$ $VALUE$ [?ROOT.power|Y2] [GetMilitiaName] $absent$"
        self.assertEqual(localise(text, {"RIGHT": "wrong", "VALUE": "wrong"}), text)
        self.assertEqual(localise("$known|+=2$", {"known": "one"}), "$known|+=2$")

    def test_cycles_are_bounded_without_losing_original_reference(self):
        self.assertEqual(localise("$A$", {"A": "$B$", "B": "$A$"}), "$A$")
        self.assertEqual(localise("$A$", {"A": "$A$"}), "$A$")
        explosive = localise("$A$", {"A": "x$A$$A$"})
        self.assertLess(len(explosive), 100)

    def test_depth_and_length_budgets(self):
        labels = {f"K{i}": f"$K{i+1}$" for i in range(100)}
        self.assertIn("$K", localise("$K0$", labels))
        result = localise("$long$", {"long": "x" * (MAX_LOCALISATION_LENGTH * 3)})
        self.assertLessEqual(len(result), MAX_LOCALISATION_LENGTH)
        self.assertIn("Localizzazione troncata", result)

    def test_reference_expansion_budget_leaves_remaining_tokens_visible(self):
        result = localise("$K$ " * 600, {"K": "done"})
        self.assertEqual(result.count("done"), 512)
        self.assertEqual(result.count("$K$"), 88)

    def test_plain_text_escaping_is_preserved_not_executed_or_decoded(self):
        text = r'\$KEY$ <script>alert("x")</script> &lt;img&gt; £trigger_yes£'
        self.assertEqual(localise(text, {"KEY": "expanded"}), text)


class FocusTooltipTest(unittest.TestCase):
    def test_direct_tooltips_and_known_numeric_rewards(self):
        node = parse('''available={ custom_trigger_tooltip={tooltip=REQ tag=AUS} }
                        bypass={ tooltip=BYPASS }
                        completion_reward={add_political_power=50 add_stability=0.025
                          custom_effect_tooltip=EFFECT show_ideas_tooltip=ADVISOR}''')
        result = focus_tooltips(node, {"REQ": "Serve $NAME_STATIC$", "NAME_STATIC": "Austria",
                                      "BYPASS": "Già ottenuto", "EFFECT": "§GEffetto§!", "ADVISOR": "Bruno"})
        self.assertEqual([s["heading"] for s in result], ["Requisiti", "Bypass", "Effetti"])
        self.assertEqual(result[0]["text"], "Serve Austria")
        self.assertIn("Potere politico: +50", result[2]["text"])
        self.assertIn("Stabilità: +2.5%", result[2]["text"])
        self.assertIn("Idea o consigliere: Bruno", result[2]["text"])

    def test_conditional_rewards_are_not_flattened(self):
        node = parse('''completion_reward={custom_effect_tooltip=DIRECT
                          if={limit={has_war=yes} custom_effect_tooltip=WAR}
                          ELSE={custom_effect_tooltip=PEACE}
                          hidden_effect={custom_effect_tooltip=SECRET}}''')
        text = focus_tooltips(node, {"DIRECT": "Sempre visibile", "WAR": "Testo guerra",
                                     "PEACE": "Testo pace", "SECRET": "Testo nascosto"})[0]["text"]
        self.assertIn("Sempre visibile", text)
        for forbidden in ("Testo guerra", "Testo pace", "Testo nascosto"):
            self.assertNotIn(forbidden, text)
        self.assertIn("Altri effetti condizionali da verificare nel gioco", text)

    def test_unresolved_script_and_nonfinite_values_do_not_become_rewards(self):
        node = parse('''completion_reward={unknown_effect=yes add_manpower=nan
                         army_experience=1e999999999 custom_effect_tooltip=missing}''')
        self.assertEqual(focus_tooltips(node, {}), [{"heading": "Effetti", "text": "Altri effetti da verificare nel gioco"}])

    def test_empty_blocks_and_spacing_tooltips_do_not_add_noise(self):
        node = parse('available={} bypass={} completion_reward={custom_effect_tooltip=generic_skip_one_line_tt}')
        self.assertEqual(focus_tooltips(node, {}), [])

    def test_nested_requirements_do_not_claim_runtime_state(self):
        node = parse('available={OR={has_war=yes custom_trigger_tooltip={tooltip=INNER}}}')
        self.assertEqual(focus_tooltips(node, {"INNER": "Da non appiattire"}),
                         [{"heading": "Requisiti", "text": "Condizioni aggiuntive da verificare nel gioco"}])


if __name__ == "__main__":
    unittest.main()
