'use strict';
/**
 * Read-only, weakly supervised classifier for Word ACTION-column instructions.
 * It proposes a semantic domain; it NEVER creates or changes a guide step.
 * Quantized weights are trained data, not Weight-Space Programming.
 */
((root) => {
  function decode(value) {
    if (typeof value !== 'string') throw Error('Invalid weight encoding');
    if (typeof Buffer !== 'undefined') return Uint8Array.from(Buffer.from(value, 'base64'));
    const raw = atob(value), bytes = new Uint8Array(raw.length);
    for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
    return bytes;
  }
  function load(model) {
    if (!model || model.schema_version !== 1 || model.domain !== 'ACTION' ||
        !Array.isArray(model.labels) || model.labels.length !== 7 ||
        !Array.isArray(model.tokens) || model.tokens.length > 5000 ||
        !Array.isArray(model.weight_scales) || model.weight_scales.length !== model.labels.length ||
        !Array.isArray(model.bias) || model.bias.length !== model.labels.length) {
      throw Error('Unsupported guide intent model');
    }
    const size = model.tokens.length, idf = decode(model.idf_u8), weights = decode(model.weights_i8);
    if (idf.length !== size || weights.length !== size * model.labels.length ||
        model.tokens.some(x => typeof x !== 'string' || x.length < 2 || x.length > 5) ||
        model.weight_scales.some(x => !Number.isFinite(x) || x <= 0) ||
        model.bias.some(x => !Number.isFinite(x)) ||
        !Number.isFinite(model.idf_scale) || model.idf_scale <= 0) {
      throw Error('Invalid guide intent matrix');
    }
    const vocabulary = new Map(model.tokens.map((token, index) => [token, index]));
    function predict(input) {
      const text = String(input || '').toLowerCase().replace(/\s\s+/g, ' ').slice(0, 2000);
      const counts = new Map();
      for (let n = 2; n <= 5; n++) for (let i = 0; i <= text.length - n; i++) {
        const index = vocabulary.get(text.slice(i, i + n));
        if (index !== undefined) counts.set(index, (counts.get(index) || 0) + 1);
      }
      let norm2 = 0;
      const features = [];
      for (const [i, c] of counts) {
        const value = (1 + Math.log(c)) * idf[i] / model.idf_scale;
        norm2 += value * value;
        features.push([i, value]);
      }
      const norm = Math.sqrt(norm2) || 1;
      const ranking = model.labels.map((label, row) => {
        let score = model.bias[row];
        for (const [index, value] of features) {
          const byte = weights[row * size + index];
          const signed = byte > 127 ? byte - 256 : byte;
          score += signed * model.weight_scales[row] * value / norm;
        }
        return {label, score};
      }).sort((a, b) => b.score - a.score);
      const margin = ranking[0].score - ranking[1].score;
      return {label: ranking[0].label, margin, runner_up: ranking[1].label,
        features: features.length, requires_review: features.length < 3 || margin < 0.35,
        model: model.source_model};
    }
    return Object.freeze({predict, labels: Object.freeze([...model.labels])});
  }
  const api = {load};
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else root.HoiGuideIntent = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
