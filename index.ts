import { compile } from "handlebars";
import "dotenv/config";

const DB_FILE = "data.json";

interface DataFile {
  lastTransactionTimestamp: number;
  balance: number;
  users: Record<Username, number>;
  transactions: LocalTransaction[];
}

interface LocalTransaction {
  amount: number;
  timestamp: number;
  user: Username;
  action: TransactionAction;
}

type Uuid = string & { readonly _sym: unique symbol; };
type Username = string & { readonly _sym: unique symbol; };
type StyledUsername = string & { readonly _sym: unique symbol; };

type HypixelResponse<T> = ({ success: true; } & T) | { success: false; cause: string };
type ProfileResponse = HypixelResponse<{ profile: Profile }>;

interface Profile {
  profile_id: string;
  community_upgrades: CommunityUpgrades;
  created_at: number;
  members: Record<Uuid, Member>;
  banking: Banking;
}

interface Member { /* TODO */ }

interface CommunityUpgrades { /* TODO */ }

interface Banking {
  balance: number;
  transactions: Transaction[]
}

interface Transaction {
  amount: number;
  timestamp: number;
  action: TransactionAction;
  initiator_name: StyledUsername;
}

enum TransactionAction {
  Deposit = "DEPOSIT",
  Withdraw = "WITHDRAW",
}

async function fetchApi() {
  console.log(`Fetching fresh information for profile '${process.env.PROFILE_UUID!}'`);

  let url = new URL("https://api.hypixel.net/v2/skyblock/profile");
  url.searchParams.append("key", process.env.API_KEY!);
  url.searchParams.append("profile", process.env.PROFILE_UUID!);

  let data: ProfileResponse = await fetch(url).then((res) => res.json());
  if (!data.success) throw new Error(`There was an error while fetching Hypixel's API: ${data.cause}`);

  updateTransactions(data.profile.banking);
}

async function updateTransactions(banking: Banking) {
  console.log("TSC: Updating");

  const db: DataFile = await Bun.file(DB_FILE).json();

  let newTransactions: LocalTransaction[] = banking.transactions
    .filter((transac) => transac.timestamp > (db.lastTransactionTimestamp ?? 0))
    .map(transac => ({
      action: transac.action,
      amount: transac.amount,
      timestamp: transac.timestamp,
      user: processDisplayUsername(transac.initiator_name)
    }));

  db.transactions = db.transactions.concat(newTransactions);

  for (const transaction of newTransactions) {
    console.log(`TSC NEW: ${transaction.user} has ${transaction.action} ${transaction.amount} coins`)

    if (!db.users[transaction.user]) {
      db.users[transaction.user] = 0;
    }

    switch (transaction.action) {
      case TransactionAction.Deposit:
        db.users[transaction.user] += transaction.amount;
        break;
      case TransactionAction.Withdraw:
        db.users[transaction.user] -= transaction.amount;
        break;
    }

    db.lastTransactionTimestamp = transaction.timestamp;
  }

  if (newTransactions.length >= 50) console.warn("TSC WARN: there are 50 new transactions, maybe some were not correctly registered");

  let sum = Object.values(db.users).reduce((sum, a) => sum + a, 0);
  let drift = Math.abs(banking.balance - sum);
  console.log(`TSC DRIFT: found ${drift} (balance ${banking.balance}, sum ${sum})`)

  db.balance = banking.balance;

  await Bun.write(DB_FILE, JSON.stringify(db));
}

function processDisplayUsername(display: StyledUsername): Username {
  if (display === "Bank Interest") {
    return "Bank Interest" as Username;
  } else {
    // Strip `§a` Minecraft display style tag
    return display.slice(2) as Username
  }
}


async function renderHtml() {
  const db: DataFile = await Bun.file(DB_FILE).json();

  let template = compile<DataFile>(await Bun.file("index.html.hbs").text());

  let helpers = {
    formatDateTime(date: number) {
      return new Intl.DateTimeFormat('fr-FR', { month: "numeric", day: "numeric", hour: "numeric", minute: "numeric" }).format(date);
    },
    isDeposit: (arg: string) => (arg === TransactionAction.Deposit),
  }

  let html = template({ ...db, transactions: db.transactions.reverse() }, { helpers });
  return new Response(html, { headers: { "Content-Type": "text/html" } });
}

Bun.serve({
  async fetch(request, server) {
    const url = new URL(request.url);
    if (url.pathname === "/") return renderHtml();
    if (url.pathname === "/styles.css") return new Response(Bun.file("styles.css"));
    return new Response("404!", { status: 404 });
  },
})

// Fetch once on startup and then fetch every 10m
fetchApi();
setInterval(fetchApi, 10 * 60 * 1000)
