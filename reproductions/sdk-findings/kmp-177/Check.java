// Soneso/kmp-stellar-sdk#177: fromEnvelopeXdr refuses four kinds of
// transaction the network applied. Inputs: the testnet transactions in
// ../witnesses.json (all txSUCCESS).
//
// Usage: ../kmp-175/fetch.sh && java -cp '../kmp-175/.deps/*' Check.java

import com.soneso.stellar.sdk.AbstractTransaction;
import com.soneso.stellar.sdk.Network;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class Check {
  static final List<String> SHAPES = List.of(
      "manage-data-space-name", "nested-not-predicate", "precond-max-ledger-u32", "revoke-pool-share-sponsorship");

  public static void main(String[] args) throws Exception {
    String json = Files.readString(Path.of("../witnesses.json"));
    // Each transaction object lists "shape" before "envelope".
    Matcher m = Pattern.compile("\"shape\": \"([^\"]+)\"[^}]*?\"envelope\": \"([^\"]+)\"").matcher(json);
    int refused = 0;
    while (m.find()) {
      if (!SHAPES.contains(m.group(1))) continue;
      try {
        AbstractTransaction.Companion.fromEnvelopeXdr(m.group(2), new Network("Test SDF Network ; September 2015"));
        System.out.println(m.group(1) + ": parsed");
      } catch (RuntimeException e) {
        System.out.println(m.group(1) + ": refused: " + e.getMessage());
        refused++;
      }
    }
    System.exit(refused == 0 ? 0 : 1);
  }
}
