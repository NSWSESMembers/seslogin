/**
 * @generated SignedSource<<ef6c25853292dcf67ca1ff2f0b67adce>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SettingsActivityDisplayMutation$variables = {
  value: boolean;
};
export type SettingsActivityDisplayMutation$data = {
  readonly updateMyDisaggregateVirtualPeriods: {
    readonly disaggregateVirtualPeriods: boolean;
  };
};
export type SettingsActivityDisplayMutation = {
  response: SettingsActivityDisplayMutation$data;
  variables: SettingsActivityDisplayMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "value"
  }
],
v1 = [
  {
    "kind": "Variable",
    "name": "value",
    "variableName": "value"
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "disaggregateVirtualPeriods",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SettingsActivityDisplayMutation",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "updateMyDisaggregateVirtualPeriods",
        "plural": false,
        "selections": [
          (v2/*: any*/)
        ],
        "storageKey": null
      }
    ],
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SettingsActivityDisplayMutation",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "updateMyDisaggregateVirtualPeriods",
        "plural": false,
        "selections": [
          (v2/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "id",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "99c4c48eefdc39887657e1e848aac24e",
    "id": null,
    "metadata": {},
    "name": "SettingsActivityDisplayMutation",
    "operationKind": "mutation",
    "text": "mutation SettingsActivityDisplayMutation(\n  $value: Boolean!\n) {\n  updateMyDisaggregateVirtualPeriods(value: $value) {\n    disaggregateVirtualPeriods\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "a78001a0cc9e32ab563b2513d536643b";

export default node;
