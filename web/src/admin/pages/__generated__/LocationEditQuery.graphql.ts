/**
 * @generated SignedSource<<14cbff1f90a5114067ed461e4b29896d>>
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
    readonly nitcCompleteOnExport: boolean;
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
        "name": "nitcCompleteOnExport",
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
    "metadata": {
      "throwOnFieldError": true
    },
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
    "cacheID": "36376905a839fcefc22f6eef2fcc66bd",
    "id": null,
    "metadata": {},
    "name": "LocationEditQuery",
    "operationKind": "query",
    "text": "query LocationEditQuery(\n  $id: ID!\n) {\n  location(id: $id) {\n    id\n    name\n    enabled\n    nitcEnabled\n    nitcCompleteOnExport\n    gamificationEnabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "fa71ede7f7d7514e3621cdc6e9d2e772";

export default node;
