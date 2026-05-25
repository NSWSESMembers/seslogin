/**
 * @generated SignedSource<<ff35e4cc46ea8358c19779ab69c78031>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsPasskeysRenameMutation$variables = {
  id: string;
  name: string;
};
export type SettingsPasskeysRenameMutation$data = {
  readonly renamePasskey: {
    readonly id: string;
    readonly name: string;
  };
};
export type SettingsPasskeysRenameMutation = {
  response: SettingsPasskeysRenameMutation$data;
  variables: SettingsPasskeysRenameMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  },
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "name"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      }
    ],
    "concreteType": "PasskeyInfo",
    "kind": "LinkedField",
    "name": "renamePasskey",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "name",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SettingsPasskeysRenameMutation",
    "selections": (v1/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SettingsPasskeysRenameMutation",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "63bc7ec9f496b9a412b0c734331a2ec5",
    "id": null,
    "metadata": {},
    "name": "SettingsPasskeysRenameMutation",
    "operationKind": "mutation",
    "text": "mutation SettingsPasskeysRenameMutation(\n  $id: String!\n  $name: String!\n) {\n  renamePasskey(id: $id, name: $name) {\n    id\n    name\n  }\n}\n"
  }
};
})();

(node as any).hash = "8b7d47ff824f05171dd08bb4aeeb4e22";

export default node;
