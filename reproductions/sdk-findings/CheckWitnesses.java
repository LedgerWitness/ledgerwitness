// Checks every transaction in witnesses.json with java-stellar-sdk: the
// transaction API must parse it, return the network hash and re-encode the
// original bytes. Exits 1 on any difference.
//
// Usage: java -cp 'runners/java/.deps/*' reproductions/sdk-findings/CheckWitnesses.java

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import org.stellar.sdk.AbstractTransaction;
import org.stellar.sdk.Network;

public class CheckWitnesses {
  public static void main(String[] args) throws Exception {
    JsonObject d = JsonParser.parseString(Files.readString(Path.of("reproductions/sdk-findings/witnesses.json"))).getAsJsonObject();
    Network network = new Network(d.get("network_passphrase").getAsString());
    int total = 0, bad = 0;
    for (var e : d.getAsJsonArray("transactions")) {
      JsonObject t = e.getAsJsonObject();
      total++;
      List<String> problems = new ArrayList<>();
      try {
        AbstractTransaction tx = AbstractTransaction.fromEnvelopeXdr(t.get("envelope").getAsString(), network);
        if (!tx.hashHex().equals(t.get("hash").getAsString())) problems.add("wrong hash");
        if (!tx.toEnvelopeXdrBase64().equals(t.get("envelope").getAsString())) problems.add("bytes differ");
      } catch (RuntimeException x) {
        problems.add("refused: " + x.getMessage());
      }
      if (!problems.isEmpty()) {
        bad++;
        System.out.println(t.get("shape").getAsString() + ": " + String.join(", ", problems));
      }
    }
    System.out.println("java-stellar-sdk: " + (total - bad) + "/" + total + " witnesses ok");
    System.exit(bad == 0 ? 0 : 1);
  }
}
