/* @refresh reload */
import { render } from "solid-js/web";
import { HashRouter, Route } from "@solidjs/router";
import "./app.css";
import App from "./App";
import Home from "./routes/index";
import InstanceDetail from "./routes/instance/[id]";
import NewInstance from "./routes/instance/new";
import ModBrowser from "./routes/mods";
import Accounts from "./routes/accounts";
import Settings from "./routes/settings";

render(
  () => (
    <HashRouter root={App}>
      <Route path="/" component={Home} />
      <Route path="/instance/new" component={NewInstance} />
      <Route path="/instance/:id" component={InstanceDetail} />
      <Route path="/mods" component={ModBrowser} />
      <Route path="/accounts" component={Accounts} />
      <Route path="/settings" component={Settings} />
    </HashRouter>
  ),
  document.getElementById("root") as HTMLElement,
);
