/**
 * @generated SignedSource<<85f2caf7c27e61f66bc62b1aab67818d>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionEnrollMutation$variables = {
  config?: string | null | undefined;
  healthcheckUrl?: string | null | undefined;
  keyFingerprint: string;
  locationId: string;
  name: string;
};
export type SessionEnrollMutation$data = {
  readonly enrollSession: {
    readonly id: string;
  };
};
export type SessionEnrollMutation = {
  response: SessionEnrollMutation$data;
  variables: SessionEnrollMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "config"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "healthcheckUrl"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "keyFingerprint"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "locationId"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v5 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "config",
        "variableName": "config"
      },
      {
        "kind": "Variable",
        "name": "healthcheckUrl",
        "variableName": "healthcheckUrl"
      },
      {
        "kind": "Variable",
        "name": "keyFingerprint",
        "variableName": "keyFingerprint"
      },
      {
        "kind": "Variable",
        "name": "locationId",
        "variableName": "locationId"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      }
    ],
    "concreteType": "Session",
    "kind": "LinkedField",
    "name": "enrollSession",
    "plural": false,
    "selections": [
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
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/),
      (v4/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "SessionEnrollMutation",
    "selections": (v5/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v4/*: any*/),
      (v3/*: any*/),
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/)
    ],
    "kind": "Operation",
    "name": "SessionEnrollMutation",
    "selections": (v5/*: any*/)
  },
  "params": {
    "cacheID": "5c1a0ffabdfd23157dad6b951cfc4efc",
    "id": null,
    "metadata": {},
    "name": "SessionEnrollMutation",
    "operationKind": "mutation",
    "text": "mutation SessionEnrollMutation(\n  $name: String!\n  $locationId: ID!\n  $config: String\n  $healthcheckUrl: String\n  $keyFingerprint: String!\n) {\n  enrollSession(name: $name, locationId: $locationId, config: $config, healthcheckUrl: $healthcheckUrl, keyFingerprint: $keyFingerprint) {\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "7917c436209d9776834a7dfc30616d24";

export default node;
