/**
 * @generated SignedSource<<3529bce6bbbfa1fe57b61e4872cbfc84>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsPasskeysDeleteMutation$variables = {
  id: string;
};
export type SettingsPasskeysDeleteMutation$data = {
  readonly deletePasskey: boolean;
};
export type SettingsPasskeysDeleteMutation = {
  response: SettingsPasskeysDeleteMutation$data;
  variables: SettingsPasskeysDeleteMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
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
      }
    ],
    "kind": "ScalarField",
    "name": "deletePasskey",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SettingsPasskeysDeleteMutation",
    "selections": (v1/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SettingsPasskeysDeleteMutation",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "6cbb058afb4dd8a0055bc75e61bb626c",
    "id": null,
    "metadata": {},
    "name": "SettingsPasskeysDeleteMutation",
    "operationKind": "mutation",
    "text": "mutation SettingsPasskeysDeleteMutation(\n  $id: String!\n) {\n  deletePasskey(id: $id)\n}\n"
  }
};
})();

(node as any).hash = "16cc73109c9247c4da6ef3d74b82c9bc";

export default node;
