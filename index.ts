import { compile } from "handlebars";

const DB_FILE = "./data.json";
let is_file_locked = false;

const API_KEY = "51a267a2-0701-43d2-be2e-9d8ed03485e1";
const PROFILE_UUID = "cf0499b7-45a6-4e2c-9150-ae32ec8a2b66";

interface DataFile {
  lastTransactionTimestamp?: number;
  users?: Record<Username, number>;
  transactions?: LocalTransaction[];
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

interface ProfileResponse {
  success: boolean;
  profile: Profile;
}

interface Profile {
  profile_id: string;
  community_upgrades: CommunityUpgrades;
  created_at: number;
  members: Record<Uuid, Member>;
  banking: Banking;
}

interface Member {
  // TODO
}

interface CommunityUpgrades { }

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
  console.log("Fetching new info for profile");

  let url = new URL("https://api.hypixel.net/v2/skyblock/profile");
  url.searchParams.append("key", API_KEY);
  url.searchParams.append("profile", PROFILE_UUID);

  let res = await fetch(url);

  if (!res.ok) {
    console.error("Could not properly fetch API!")
  }

  let data: ProfileResponse = await res.json();

  if (!data.success) {
    console.error("200 yet hypixel is taunting")
  }

  let profile = data.profile;

  updateTransactions(profile.banking);
}

async function updateTransactions(banking: Banking) {
  console.log("Updating user transactions");
  while (is_file_locked) { }
  is_file_locked = true;
  const db: DataFile = await Bun.file(DB_FILE).json();

  if (!db.lastTransactionTimestamp) db.lastTransactionTimestamp = 0;
  if (!db.users) db.users = {};
  if (!db.transactions) db.transactions = [];

  let new_transac: LocalTransaction[] = banking.transactions
    .filter((transac) => transac.timestamp > db.lastTransactionTimestamp)
    .map(transac => ({
      action: transac.action,
      amount: transac.amount,
      timestamp: transac.timestamp,
      user: processDisplayUsername(transac.initiator_name)
    }));

  db.transactions = db.transactions.concat(new_transac);

  for (const transaction of new_transac) {
    console.log(`TRANSAC: ${transaction.user} has ${transaction.action} ${transaction.amount} coins`)

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

  await Bun.write(DB_FILE, JSON.stringify(db));
  is_file_locked = false;
}

function processDisplayUsername(display: StyledUsername): Username {
  if (display === "Bank Interest") {
    return "Bank Interest" as Username;
  } else {
    return display.slice(2) as Username
  }
}

fetchApi();
setInterval(fetchApi, 10 * 60 * 1000)

Bun.serve({
  port: 3000,
  async fetch(request, server) {
    const db: DataFile = await Bun.file(DB_FILE).json();

    let template = compile(`<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Bank Account Tracker</title>
    <link rel="stylesheet" href="style.css">
</head>
<body>
    <style>
    body {
    background-color: #999999;
    font-family: Arial, sans-serif;
    margin: 0;
    padding: 0;
    display: flex;
}

h1 {
    text-align: center;
}

.transaction-history li {
    list-style-type: none;
    margin: 1%;
}

.transaction-history {
    width: 50%;
    margin: 0 auto;
    padding: 20px;
    margin: 20px;
    background-color: #cccccc;
    border-radius: 5px;
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
}

.balance {
    width: 50%;
    margin: 0 auto;
    padding: 20px;
    margin: 20px;
    background-color: #cccccc;
    border-radius: 5px;
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.1);
}

.total {
    font-size: 150%;
    text-align: center;
}

.members {
    margin-top: 5%;
}

.members-list {
    padding: 0%;
    margin: 0 auto;
}

.members-list li {
    list-style-type: none;
    font-size: 120%;
    margin: 1%;
}
    </style>
    <div class="transaction-history">
        <h1>Transactions history</h1>
        <ol class="history-list">
          {{#each transactions}}
            <li>{{this.action}} {{this.amount}} — {{this.user}} ({{this.timestamp}}) </li>
          {{/each}}
        </ol>
    </div>
    <div class="balance">
        <h1>Account balance</h1>
        <p class="total">Total: X coins</p>
        <h2 class="members">Members list</h2>
        <ul class="members-list">
          {{#each users}}
            <li>{{@key}}: {{this}} coins</li>
          {{/each}}
        </ul>
    </div>
</body>
</html>
    `);

    let html = template({ transactions: db.transactions?.reverse(), users: db.users });
    return new Response(html, { headers: { "Content-Type": "text/html" } });
  },
})
