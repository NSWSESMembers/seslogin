/**
 * @generated SignedSource<<37566c945be0413720c674aa96ce9cd1>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type KioskRealtimeTokenQuery$variables = Record<PropertyKey, never>;
export type KioskRealtimeTokenQuery$data = {
  readonly kioskRealtimeToken: {
    readonly channel: string;
    readonly sessionChannel: string | null | undefined;
    readonly tokenRequest: {
      readonly capability: string;
      readonly clientId: string;
      readonly keyName: string;
      readonly mac: string;
      readonly nonce: string;
      readonly timestamp: number;
      readonly ttl: number;
    };
  } | null | undefined;
};
export type KioskRealtimeTokenQuery = {
  response: KioskRealtimeTokenQuery$data;
  variables: KioskRealtimeTokenQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "KioskRealtimeToken",
    "kind": "LinkedField",
    "name": "kioskRealtimeToken",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "channel",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "sessionChannel",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "AblyTokenRequest",
        "kind": "LinkedField",
        "name": "tokenRequest",
        "plural": false,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "keyName",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "ttl",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "capability",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "clientId",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "timestamp",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "nonce",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "mac",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": null,
    "name": "KioskRealtimeTokenQuery",
    "selections": (v0/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "KioskRealtimeTokenQuery",
    "selections": (v0/*:: as any*/)
  },
  "params": {
    "cacheID": "47bef459f4aa753d1dd670353390e361",
    "id": null,
    "metadata": {},
    "name": "KioskRealtimeTokenQuery",
    "operationKind": "query",
    "text": "query KioskRealtimeTokenQuery {\n  kioskRealtimeToken {\n    channel\n    sessionChannel\n    tokenRequest {\n      keyName\n      ttl\n      capability\n      clientId\n      timestamp\n      nonce\n      mac\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "5d76eb4d511bb0637e5359d578b79287";

export default node;
