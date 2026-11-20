// Strkey decoder check for reproductions/strkey (Java): prints, for every
// vector, whether java-stellar-sdk's StrKey accepts it and whether it
// re-encodes unchanged.
//
// Usage: java -cp 'runners/java/.deps/*' reproductions/strkey/Check.java <vectors.json>

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonParser;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;
import org.stellar.sdk.StrKey;
import org.stellar.sdk.Transaction;

public class Check {
  record Codec(Function<String, byte[]> decode, Function<byte[], String> encode) {}

  static final Map<Character, Codec> CODECS = Map.of(
      'G', new Codec(StrKey::decodeEd25519PublicKey, StrKey::encodeEd25519PublicKey),
      'M', new Codec(StrKey::decodeMed25519PublicKey, StrKey::encodeMed25519PublicKey),
      'P', new Codec(StrKey::decodeSignedPayload, StrKey::encodeSignedPayload),
      'B', new Codec(StrKey::decodeClaimableBalance, StrKey::encodeClaimableBalance),
      'C', new Codec(StrKey::decodeContract, StrKey::encodeContract),
      'W', new Codec(StrKey::decodeMuxedContract, StrKey::encodeMuxedContract),
      'L', new Codec(StrKey::decodeLiquidityPool, StrKey::encodeLiquidityPool),
      'T', new Codec(StrKey::decodePreAuthTx, StrKey::encodePreAuthTx),
      'X', new Codec(StrKey::decodeSha256Hash, StrKey::encodeSha256Hash),
      'S', new Codec(
          s -> StrKey.decodeEd25519SecretSeed(s.toCharArray()),
          b -> new String(StrKey.encodeEd25519SecretSeed(b))));

  static String version() {
    String p = Transaction.class.getProtectionDomain().getCodeSource().getLocation().getPath();
    return Paths.get(p).getFileName().toString().replaceFirst("^stellar-sdk-", "").replaceFirst("\\.jar$", "");
  }

  public static void main(String[] args) throws Exception {
    List<Object> out = new ArrayList<>();
    for (JsonElement e : JsonParser.parseString(Files.readString(Paths.get(args[0]))).getAsJsonArray()) {
      String strkey = e.getAsJsonObject().get("strkey").getAsString();
      Map<String, Object> r = new LinkedHashMap<>();
      r.put("strkey", strkey);
      // Dispatch on the upper-cased prefix, as the other checkers do, so that
      // a lower-case strkey reaches the decoder that must reject it.
      Codec c = strkey.isEmpty() ? null : CODECS.get(Character.toUpperCase(strkey.charAt(0)));
      if (c == null) {
        r.put("verdict", "unsupported");
      } else {
        try {
          byte[] raw = c.decode().apply(strkey);
          String back;
          try {
            back = c.encode().apply(raw);
          } catch (RuntimeException ex) {
            back = null;
          }
          r.put("verdict", "accept");
          r.put("roundtrip", strkey.equals(back));
        } catch (RuntimeException ex) {
          String m = String.valueOf(ex.getMessage());
          r.put("verdict", "reject");
          r.put("error", m.length() > 80 ? m.substring(0, 80) : m);
        }
      }
      out.add(r);
    }
    Map<String, Object> report = new LinkedHashMap<>();
    report.put("sdk", "java " + version());
    report.put("results", out);
    System.out.println(new Gson().toJson(report));
  }
}
