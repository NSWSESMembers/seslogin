/**
 * @generated SignedSource<<d7c8248d6f1e386b4a769e93c34424f3>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type LocationEditQuery$variables = {
  id: string;
};
export type LocationEditQuery$data = {
  readonly location: {
    readonly enabled: boolean;
    readonly gamificationEnabled: boolean;
    readonly id: string;
    readonly name: string;
    readonly nitcEnabled: number | null | undefined;
  };
};
export type LocationEditQuery = {
  response: LocationEditQuery$data;
  variables: LocationEditQuery$variables;
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
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "location",
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
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "enabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcEnabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "gamificationEnabled",
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
    "name": "LocationEditQuery",
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "LocationEditQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "e3226633d4dd8367639a74b5fe011fdc",
    "id": null,
    "metadata": {},
    "name": "LocationEditQuery",
    "operationKind": "query",
    "text": "query LocationEditQuery(\n  $id: ID!\n) {\n  location(id: $id) {\n    id\n    name\n    enabled\n    nitcEnabled\n    gamificationEnabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "ac36e942e7ac7792e3dee0b805b02b6b";

export default node;
